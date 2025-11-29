# Release Process

## Steps to create a release

1. **Update version in Cargo.toml**
   - Bump the version field following semver (major.minor.patch)
   ```toml
   version = "0.2.0"
   ```

2. **Update CHANGELOG.md**
   - Add a new section for the version with date
   - List changes under: Added, Changed, Fixed, Removed
   ```markdown
   ## [0.2.0] - 2025-MM-DD

   ### Added
   - New feature descriptions

   ### Changed
   - Modified behavior descriptions

   ### Fixed
   - Bug fix descriptions
   ```

3. **Run full test suite**
   ```bash
   pls run ci
   # Or manually:
   cargo test --all-features
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

4. **Commit changes**
   ```bash
   git add Cargo.toml Cargo.lock CHANGELOG.md
   git commit -m "chore: release vX.Y.Z"
   git push origin main
   ```

5. **Create and push git tag**
   ```bash
   git tag -a vX.Y.Z -m "Release X.Y.Z: [brief description of main changes]"
   git push origin vX.Y.Z
   ```

6. **Monitor GitHub Actions**
   - Go to repository → Actions tab
   - Watch the "Release" workflow execute
   - Should complete in ~10-15 minutes (builds for 3 platforms)

7. **Verify release artifacts**
   - Go to repository → Releases
   - Verify new vX.Y.Z release is created
   - Check all artifacts are present:
     - Linux x86_64 binary (tar.gz)
     - macOS x86_64 binary (tar.gz)
     - macOS aarch64 binary (tar.gz)
     - Shell installer script (pls-installer.sh)
     - Homebrew formula (pls.rb)
     - SHA256 checksums file

8. **Test installation**
   - Try the shell installer on a clean system:
   ```bash
   curl --proto '=https' --tlsv1.2 -LsSf \
     https://github.com/james-w/pls/releases/download/vX.Y.Z/pls-installer.sh | sh
   ```

## Generated Artifacts

Each release produces:
- **Linux x86_64** binary (tar.gz)
- **macOS x86_64** (Intel) binary (tar.gz)
- **macOS aarch64** (Apple Silicon) binary (tar.gz)
- **Shell installer** script for easy installation
- **Homebrew formula** (.rb file, for manual submission to homebrew-core)
- **SHA256 checksums** for all artifacts

## Platform Support

- ✅ Linux (x86_64)
- ✅ macOS (Intel and Apple Silicon)
- ❌ Windows (not supported - requires Unix-only dependencies: `nix`, `daemonize`)

## Notes

### Version Synchronization
**CRITICAL**: The version in `Cargo.toml` must exactly match the git tag:
- Cargo.toml: `version = "0.2.0"`
- Git tag: `v0.2.0`

Mismatch will cause the release workflow to fail.

### Publishing to crates.io
The workflow does **not** automatically publish to crates.io. To publish manually:
```bash
cargo publish
```

### Homebrew Formula
The workflow generates a Homebrew formula (`pls.rb`) but does not auto-submit it.

For personal tap:
1. Create a `homebrew-tap` repository
2. Add the formula to `Formula/pls.rb`

For official Homebrew:
1. Wait until the tool is stable (~v1.0.0)
2. Fork `homebrew/homebrew-core`
3. Add the formula from the release artifacts
4. Submit PR to `homebrew/homebrew-core`

### Test Releases
To test the release process without creating an official release:
```bash
# Create a test tag
git tag v0.1.0-rc1
git push origin v0.1.0-rc1

# Monitor the workflow
# If issues occur, delete the tag:
git tag -d v0.1.0-rc1
git push origin :v0.1.0-rc1
```

## Troubleshooting

### Workflow fails to build
- Check that tests pass locally with `pls run ci`
- Verify version in Cargo.toml matches the git tag
- Check workflow logs in Actions tab for specific errors

### No artifacts uploaded
- Verify GitHub Actions has write permissions:
  - Settings → Actions → General
  - "Workflow permissions" = "Read and write permissions"

### Build fails for specific platform
- Check the platform-specific build logs in the workflow
- For macOS: Usually cross-compilation issues
- For Linux: Usually dependency issues

### Installation script doesn't work
- Verify the release was created successfully
- Check that all artifacts were uploaded
- Test the URL manually: `curl -I <installer-url>`

## Future Enhancements

### Windows Support
To add Windows support in the future:
1. Replace `nix` crate with cross-platform alternatives
2. Replace `daemonize` with Windows services or cross-platform daemon library
3. Add conditional compilation for Unix-specific features
4. Update `dist-workspace.toml` to include Windows targets
5. Test thoroughly on Windows

### Additional Installers
cargo-dist supports additional installer types:
- npm (for JavaScript developers)
- msi (Windows installer - if Windows support is added)
- pkg (macOS installer)

To add these, update `dist-workspace.toml`:
```toml
installers = ["shell", "homebrew", "npm"]
```
