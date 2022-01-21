Two output modes:

- _Audio_: using piano samples (see `samples/readme.md`)
- _MIDI_: Only tested with Bitwig. See setup instructions below.

## Bitwig MIDI setup

1. Run `sudo modprobe snd_virmidi`
    - To have this automatically load on boot, edit `/etc/modules` and add `snd-virmidi`
2. Launch `bitwig-studio`
3. Then click the settings and add a generic controller. In the MIDI input dropdown you should see several "Virtual Raw MIDI" devices.
4. Select "Virtual Raw MIDI/1"
5. When running `dust`, press `O` to change the output to MIDI, then select the matching MIDI port.

See also: <https://github.com/anton-k/linux-audio-howto/blob/master/doc/os-setup/virtual-midi.md>
