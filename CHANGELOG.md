# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.24.5](https://github.com/tarka/xcp/compare/xcp-v0.24.4...xcp-v0.24.5) - 2026-02-06

### Other

- Fix release binary matcher.

## [0.24.4](https://github.com/tarka/xcp/compare/xcp-v0.24.3...xcp-v0.24.4) - 2026-02-06

### <!-- 4 -->Performance

- Add optimised release profile

### Other

- Add Mac to released binaries.
- Initial release-binaries workflow.
- Add license file
- Update dependencies, including a security issue with `time`

## [0.24.3](https://github.com/tarka/xcp/compare/xcp-v0.24.2...xcp-v0.24.3) - 2026-01-29

Minor maintenance release:

- Releases are now performed with release-plz.
- Add a short AI-contributions policy.

### Other

- Add release-plz config.
- Minor dependency bump.
- Ignore emacs rust-analyser settings.
- Change default branch from 'main' to 'master'
- Include AI Contribution Policy in README
- Bump dependencies.
- Add release-plz workflow.
- Fix root test.
- Update github tests with rust helper actions.
- Add emacs restore files to .gitignore.
- Minor clippy improvement.
- Remove circleci from badges.
- Remove circleci builds as they're not kept up to date currently.
