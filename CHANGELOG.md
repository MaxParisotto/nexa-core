# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.1] - 2024-02-02

### Added

- Added support for Ollama LLM server integration
- Added new LLM configuration options for server type and model selection
- Added CORS support for web integration
- Added robust JSON response handling for LLM responses
- Added comprehensive test suite for LLM functionality

### Changed

- Improved error handling in LLM client
- Enhanced test robustness with better timeout handling
- Updated system helper to handle JSON responses more reliably
- Refactored LLM client to support multiple server types

### Fixed

- Fixed JSON parsing issues in system helper
- Fixed timeout issues in LLM tests
- Fixed CORS headers configuration
- Fixed response handling for code blocks

## [1.2.0] - 2024-02-01

### Added

- Initial LLM integration with LM Studio
- Basic system helper functionality
- Task management system
- Basic test suite

### Changed

- Refactored server configuration
- Updated error handling

## [1.1.2] - 2024-01-31

### Added

- Initial release with core functionality
- Basic server implementation
- Message processing system
- Agent registry
