# Astra third-party notices

This notice covers third-party software added to Astra's browser distribution
by the XALEN analytical calculation backend. The browser bundle includes XALEN
code and the `vsop87` Rust crate described below.

The optional XALEN Hipparcos catalog is **not bundled**. Astra disables
`xalen-ephem` default features and does not include `hip-catalog`,
`xalen-stars-hip-data`, Hipparcos/Tycho catalog data, or XALEN's cloud,
Western, or Vedic layers.

## XALEN Ephemeris

Copyright 2024-2026 XALEN Technology Pvt Ltd

This product includes software developed at XALEN Technology Pvt Ltd
(https://vedika.io).

XALEN Ephemeris is licensed under the Apache License, Version 2.0. A complete
copy is distributed at
[`third_party_licenses/Apache-2.0.txt`](third_party_licenses/Apache-2.0.txt).

Relevant acknowledgments retained from XALEN's upstream `NOTICE`:

- VSOP87 planetary theory: P. Bretagnon and G. Francou (1987), Bureau des
  Longitudes.
- ELP2000-82 lunar theory: M. Chapront-Touze and J. Chapront (1982), Bureau des
  Longitudes.
- Pluto analytical theory: J. Meeus, based on the Goffin/Steyaert fit to JPL
  DE200.
- IAU 2006 precession: N. Capitaine, P.T. Wallace, and J. Chapront (2003).
- IAU 2000B nutation: D.D. McCarthy and G. Petit (IERS TN 32).
- Delta-T model: F.R. Stephenson, L.V. Morrison, and C.Y. Hohenkerk (2016).
- Fixed-star traditional names in XALEN's star-anchor code: IAU Catalog of Star
  Names, IAU Division C Working Group on Star Names, used under Creative
  Commons Attribution.

No DE440 kernel or fixed-star catalog data is distributed by Astra. DE440 is
used only as the source of fixed numeric validation expectations in tests.

## ERFA-derived coordinate routines — BSD 3-Clause

The precession and frame-bias matrix numerics in `xalen-coords`
(`pfw06`, `fw2m`, `bi00`, `bp00`, and `pmat06`) are an independent Rust port of
corresponding ERFA routines.

Copyright (C) 2013-2023, NumFOCUS Foundation.
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.
3. Neither the name of the Standards Of Fundamental Astronomy Board, the
   International Astronomical Union nor the names of its contributors may be
   used to endorse or promote products derived from this software without
   specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.

ERFA upstream: https://github.com/liberfa/erfa

## `vsop87` Rust crate — MIT License

Copyright (c) 2015-2016 Iban Eguia

The `vsop87` crate is available under MIT or Apache-2.0. Astra distributes it
under the MIT option. The complete MIT terms are distributed at
[`third_party_licenses/vsop87-MIT.txt`](third_party_licenses/vsop87-MIT.txt).
