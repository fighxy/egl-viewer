Small media files for the `av` tests, made with FFmpeg's `lavfi` sources (see D-168):

- `clip.mp4`: H.264 + AAC, 64×36, 2 s at 25 fps; red for the first second, blue for the second; a 440 Hz tone.
- `clip.webm`: the same picture and tone as VP9 + Opus.
- `rotated.mp4`: H.264, 64×36 red, with a 90° display matrix (shown 36×64).
- `silent.mp4`: H.264 48×32 green, 1 s, no audio.
- `tone.{mp3,flac,wav,ogg}`: 1.5 s of 440 Hz.
