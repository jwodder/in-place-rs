v0.2.1 (2024-07-25)
-------------------
- Changed the receiver of `InPlace::open()` from `&mut self` to `&self`
- Increased MSRV to 1.74

v0.2.0 (2023-12-22)
-------------------
- Increased MSRV to 1.70
- **Breaking**: All error types have been combined into a single
  `InPlaceError`, and all error kinds have been combined into a single
  `InPlaceErrorKind`

v0.1.0 (2023-05-17)
-------------------
Initial release
