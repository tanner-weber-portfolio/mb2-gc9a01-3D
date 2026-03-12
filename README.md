# Microbit V2 and gc9a01 Display

Tanner Weber 2026

This program uses the gc9a01 display with the microbit v2 to show a 3D object.

# Acknowledgements 

I used Bart Massey's https://github.com/pdx-cs-rust-embedded/mb2-tft-display
to get started.

# Pins

|MB2|Edge|TFT|
|-|-|-|
|p0_09|P09|RST|
|p0_10|P08|DC|
|p0_12|P01|CS|
|p0_17|P13|SCL|
|p0_13|P15|SDA|

Pot to P2

# 🚀 Build and Run

```probe-rs-tools``` is needed

```cargo embed --release```

# 📖 Writeup

# License

Copyright (C) 2026 Tanner Weber

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as
published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
