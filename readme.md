![Screenshot of the `dust` chord sequencer](shot.png)

A chord composing tool. `dust` doesn't output any audio itself but instead drives a DAW via MIDI.

## Setup

### Chord patterns

By default `dust` looks for a yaml file with chord patterns at `~/.config/dust/patterns.yaml`.

See below for more on chord progression patterns.

### Bitwig Studio MIDI

MIDI has only been tested with Bitwig, but should work with any DAW.

1. Setup virtual MIDI ports with `sudo modprobe snd_virmidi`
    - To have this automatically load on boot, edit `/etc/modules` and add `snd-virmidi`
2. Launch `bitwig-studio`
3. Setup `Dust -> Bitwig` (for sending chords/playing instruments)
    1. In `Settings > Controllers`, add a generic controller. In the MIDI input dropdown you should see several "Virtual Raw MIDI" devices.
    2. Select "Virtual Raw MIDI/1"
4. Setup `Bitwig -> Dust` (for synchronizing the clock)
    1. In `Settings > Synchronization`, find "Virtual Raw MIDI/1" and make sure both `Clock` and `Start/Stop` are active.

For me `Virtual Raw MIDI/1` corresponded to the ports called `Virtual Raw MIDI 0-0:VirMIDI 0-0 16:0`.

By default, `dust` chooses the 2nd port (i.e. port 1, when 0-indexed) for both MIDI Input and Output, which should correspond to the "Virtual Raw MIDI/1". You can change this by using the `--midi-in-port` and `--midi-out-port` arguments; just pass in the index of the port to use instead.

See also: <https://github.com/anton-k/linux-audio-howto/blob/master/doc/os-setup/virtual-midi.md>

## Usage

`dust` has two modes: "Performance" mode (default) and "Sequencer" mode. You can use `M` to switch between them.

### Performance Mode

In this mode you can bind chords to the number keys 1-9. Use e.g. `Alt-1` to select a chord to bind to the `1` key.

Alternatively, you can enter in a space-delimited progression by pressing `p`.

### Sequencer Mode

In this mode you layout chords in a sequencer format, which will run when you hit play in your DAW.

Tips:

- Use `hjkl` to move across the sequencer grid.
- Use `A` and `B` to mark sections to loop.
- Use `R` to generate a new chord progression, or `S` to generate one from a starting chord.
- With a chord selected in the grid, use `U` and `D` to browse chords.

### General tips

- Use `v` to apply a voice-leading algorithm to the chord progression. This looks for inversions that minimize finger movement across the progression.
- Use `E` to export to a MIDI file.

### Defining chord progression patterns

See `pattern.yaml`.

The chord naming system here is a little different than the conventional roman numeral system, and designed to be less ambiguous and easier to represent with ASCII text. It consists of the following parts:

1. The scale degree and mode of the chord is defined by a roman numeral. Uppercase is major, lowercase is minor.
2. Optional: `#` or `b` symbols to flatten/sharpen the chord.
3. Optional: The triad quality:
    - `+` for augmented (M3+a5)
    - `-` for diminished (m3+d5)
    - `^` for sustained 4 (P4+P5)
    - `_` for sustained 2 (M2+P5)
    - `5` for power chord (+P5)
    - If absent, is either major (M3+P5) or minor (m3+P5) depending on the roman numeral
4. Optional: After `:`, additional intervals/extensions are expressed by scale degree (relative to the mode of the chord), and comma separated (optional).
    - Degrees can be prefixed with `#` or `b` to move them up or down a step
    - These _do not stack_; i.e. if you want to have a dominant 9th it needs to be written as `V:b7,9` and not `V:9`
    - Note that this is different than conventional notation, which isn't really systematic! For example, the dominant 7th is conventionally notated as `V7`; a more straightforward notation would have this mean the major 7th. Here the dominant 7th is notated as `V:b7` and the major 7th is notated as `V:7`.
    - This lets you create e.g. cluster chords, for example `I:2`
5. Optional: Specify an inversion by either:
    - Specifying the bass scale degree after `/`
        - E.g. `III/3` sets the major 3rd to be the bass note
        - Thus e.g. `I/3` is the first inversion and `I/5` is the second inversion.
    - Specifying the inversion number after `%`
        - E.g. `III%1` is the first inversion, `III%2` is the second inversion, etc
        - Thus `I/3 == I%1`, `I/5 == I%2`, etc.
6. Optional: Shift the chord up an octave with `>1` or down an octave `<1`.
    - E.g. `I>1`
7. Optional: After `~`, specify a different relative key (you can think of this as the chord being "drawn from" that relative key)
    - E.g. `V:b7~V` is a secondary dominant (this would normally be notated `V7/V`)
    - This lets you modulate relatively easily too, e.g.: `I vi IV VI:b7 ii VI~ii iv~ii`
