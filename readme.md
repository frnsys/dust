MIDI output: Only tested with Bitwig, but should work with any DAW. See setup instructions below.

## Bitwig MIDI setup

1. Setup virtual MIDI ports with `sudo modprobe snd_virmidi`
    - To have this automatically load on boot, edit `/etc/modules` and add `snd-virmidi`
2. Launch `bitwig-studio`
3. Then click the settings and add a generic controller. In the MIDI input dropdown you should see several "Virtual Raw MIDI" devices.
4. Select "Virtual Raw MIDI/1"
5. When running `dust`, press `O` to change the output to MIDI, then select the matching MIDI port.

See also: <https://github.com/anton-k/linux-audio-howto/blob/master/doc/os-setup/virtual-midi.md>

## Defining chord progression patterns

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
4. Optional: After `:`, additional intervals are expressed by scale degree (relative to the mode of the chord), and comma separated (optional).
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

