# Third Party Licenses

This project includes third-party dependencies with various licenses. Below is a summary of the licenses for dependencies that require attribution or have specific license terms.

## MPL-2.0 Licensed Dependencies

### option-ext

- **Version**: 0.2.0
- **License**: Mozilla Public License 2.0 (MPL-2.0)
- **Repository**: https://github.com/soc/option-ext
- **Description**: Extends `Option` with additional operations
- **Usage**: Transitive dependency via `dirs` → `dirs-sys` → `option-ext`
- **Note**: No modifications have been made to the original source code. This dependency is used as-is in binary form.

#### MPL-2.0 License Summary

The MPL-2.0 is a "file-level" copyleft license. It allows the use of MPL-licensed code in projects with different licenses (including MIT) under the following conditions:

1. **No modification required**: If you use MPL-2.0 code without modification, you can distribute it under your project's license.
2. **Modification disclosure**: If you modify MPL-2.0 licensed files, those specific modified files must remain under MPL-2.0.
3. **Larger work**: You may combine MPL-2.0 code with code under other licenses in a "Larger Work" and distribute the Larger Work under different terms.

For the full license text, see: https://www.mozilla.org/en-US/MPL/2.0/

---

## Other Dependencies

All other dependencies in this project are licensed under permissive licenses (MIT, Apache-2.0, or dual MIT/Apache-2.0), which are fully compatible with this project's MIT license.

To view the complete list of dependencies and their licenses, run:

```bash
cd skilllite && cargo license
```

---

## License Compatibility

| Dependency License | Compatible with MIT | Notes |
|-------------------|---------------------|-------|
| MIT | ✅ Yes | Fully compatible |
| Apache-2.0 | ✅ Yes | Fully compatible |
| MIT OR Apache-2.0 | ✅ Yes | Dual license, choose MIT |
| MPL-2.0 | ✅ Yes | Compatible for unmodified use |
| Unlicense | ✅ Yes | Public domain equivalent |

---

*This file was generated to ensure license compliance. Last updated: 2026-01*
