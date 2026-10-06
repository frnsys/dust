mod app;
mod core;
mod file;
mod midi;
mod progression;
mod strum;

use clap::{Parser, Subcommand, ValueHint};
use std::{fs::File, path::{Path, PathBuf}, env};
use std::{io, io::BufReader};
use app::{App, run_app};
use anyhow::{Result, Context, bail};
use ratatui::crossterm::{
    execute,
    event::{DisableMouseCapture, EnableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
};
use core::{Key, Mode, ChordSpec};
use progression::ProgressionTemplate;
use strum::{StrumLibrary, NoteEvent, override_params, render_strummed, render_block};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Chord progression patterns file (default: ~/.config/dust/patterns.yaml)
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    patterns: Option<PathBuf>,

    /// Strum patterns file (default: ~/.config/dust/strums.yaml, else the built-in library)
    #[arg(long, value_hint = ValueHint::FilePath)]
    strums: Option<PathBuf>,

    #[arg(short, long, default_value = "/tmp/", value_hint = ValueHint::DirPath)]
    save_dir: String,

    /// Index of an existing MIDI input port to receive clock from.
    /// By default a virtual input port is created instead.
    #[arg(long)]
    midi_in_port: Option<usize>,

    /// Index of an existing MIDI output port to send notes to.
    /// By default a virtual output port is created instead.
    #[arg(long)]
    midi_out_port: Option<usize>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Render a chord progression to a MIDI file without the UI,
    /// e.g. for listening to strum patterns.
    Render(RenderArgs),
}

#[derive(clap::Args, Debug)]
struct RenderArgs {
    /// Space-separated chords, one per step (e.g. "I IV V vi").
    /// Use "." for a step that holds the previous chord.
    progression: String,

    /// Output MIDI file
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    out: PathBuf,

    /// Name of the strum pattern to use. Without one, block chords are rendered.
    #[arg(long)]
    strum: Option<String>,

    /// Strum pattern given directly instead of by name, e.g. "d. du .u du"
    #[arg(long, conflicts_with = "strum")]
    pattern: Option<String>,

    #[arg(long, default_value_t = 100.0)]
    bpm: f64,

    /// Number of steps per beat (quarter note)
    #[arg(long, default_value_t = 1)]
    steps_per_beat: usize,

    /// How many times to repeat the progression
    #[arg(long, default_value_t = 2)]
    repeats: usize,

    /// Random seed for the humanization; random if not given
    #[arg(long)]
    seed: Option<u64>,

    /// Root note of the key, e.g. C4 or A3
    #[arg(long, default_value = "C4")]
    root: String,

    #[arg(long)]
    minor: bool,

    /// Turn off all randomness and feel (for A/B comparison)
    #[arg(long)]
    quantized: bool,

    /// Override a strum parameter, e.g. --set spread_ms=60 (repeatable)
    #[arg(long = "set", value_name = "KEY=VALUE")]
    overrides: Vec<String>,

    /// General MIDI program number (0-127) to select at the start of the file,
    /// e.g. 24 for nylon guitar, 25 for steel guitar
    #[arg(long)]
    program: Option<u8>,

    /// Silence before the first beat, so the first strum isn't cut off
    /// when it starts slightly ahead of the beat
    #[arg(long, default_value_t = 0.0)]
    lead_in_ms: f64,
}

fn config_path(name: &str) -> PathBuf {
    let home = env::var("HOME").unwrap_or_default();
    Path::new(&home).join(".config/dust").join(name)
}

fn load_patterns(path: Option<PathBuf>) -> Result<ProgressionTemplate> {
    let path = path.unwrap_or_else(|| config_path("patterns.yaml"));
    let file = File::open(&path).with_context(|| format!("could not open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut template: ProgressionTemplate = serde_yaml::from_reader(reader)
        .with_context(|| format!("error while reading {}", path.display()))?;
    template.update_transitions();
    Ok(template)
}

fn load_strums(path: Option<PathBuf>) -> Result<StrumLibrary> {
    let path = path.unwrap_or_else(|| config_path("strums.yaml"));
    if path.exists() {
        let yaml = std::fs::read_to_string(&path)?;
        StrumLibrary::from_yaml(&yaml).with_context(|| format!("error while reading {}", path.display()))
    } else {
        Ok(StrumLibrary::default_library())
    }
}

fn render(args: RenderArgs, library: StrumLibrary) -> Result<()> {
    let key = Key {
        root: args.root.as_str().try_into().map_err(|_| anyhow::anyhow!("invalid root note `{}`", args.root))?,
        mode: if args.minor { Mode::Minor } else { Mode::Major },
    };
    let mut steps = vec![];
    for token in args.progression.split_whitespace() {
        if token == "." {
            steps.push(None);
        } else {
            let cs: ChordSpec = token.try_into().map_err(|_| anyhow::anyhow!("invalid chord `{}`", token))?;
            steps.push(Some(cs.chord_for_key(&key).midi_notes()));
        }
    }
    if steps.is_empty() {
        bail!("empty progression");
    }

    let strum = match (&args.strum, &args.pattern) {
        (Some(name), _) => {
            let preset = library.get(name)
                .with_context(|| format!("no strum pattern named `{}`; available: {}", name, library.names().join(", ")))?;
            Some((preset.pattern.clone(), preset.params.clone()))
        }
        (None, Some(pattern)) => Some((pattern.parse()?, library.defaults.clone())),
        (None, None) => None,
    };

    let events = match strum {
        Some((pattern, params)) => {
            let mut params = override_params(&params, &args.overrides)?;
            if args.quantized {
                params = params.quantize();
            }
            let seed = args.seed.unwrap_or_else(rand::random);
            eprintln!("seed: {}", seed);
            render_strummed(&steps, args.steps_per_beat, &pattern, &params, args.bpm, args.repeats, seed)
        }
        None => {
            let step_ms = 60_000.0 / args.bpm / args.steps_per_beat as f64;
            render_block(&steps, args.steps_per_beat, args.bpm, step_ms, 100, args.repeats)
        }
    };
    let events: Vec<NoteEvent> = events.into_iter()
        .map(|e| NoteEvent { at_ms: e.at_ms + args.lead_in_ms, ..e })
        .collect();
    file::save_to_midi_file_with_program(args.bpm, &events, args.program, args.out.to_string_lossy().to_string())?;
    eprintln!("wrote {} ({} note events)", args.out.display(), events.len());
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let strum_library = load_strums(args.strums)?;

    if let Some(Command::Render(render_args)) = args.command {
        return render(render_args, strum_library);
    }

    let template = load_patterns(args.patterns)?;

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(
        template,
        strum_library,
        args.midi_in_port,
        args.midi_out_port,
        args.save_dir,
    );
    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}
