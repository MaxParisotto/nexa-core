# Actual dir tree at 13 February 2025

max@MacBookPro nexa-core % tree
.
├── CHANGELOG.md
├── Cargo.lock
├── Cargo.toml
├── Cross.toml
├── Dockerfile.musl
├── PROJECT_STATUS.md
├── README.md
├── benches
│   ├── cluster_bench.rs
│   └── loadbalancer_bench.rs
├── build.sh
├── config.example.yml
├── config.yml
├── docs
│   ├── architecture.md
│   ├── openapi.yaml
│   └── swagger.html
├── release
│   ├── 1.2.0
│   │   └── nexa
│   ├── 1.2.1
│   │   └── nexa
│   ├── nexa
│   ├── nexa-1.1.0
│   ├── nexa-1.1.1
│   ├── nexa-latest -> nexa-1.1.1
│   ├── tutorial.md
│   └── v1.0.0.md
├── scripts
│   └── build_and_test.sh
├── src
│   ├── agent_types
│   │   ├── deepseek
│   │   │   └── generation.rs
│   │   └── deepseek.rs
│   ├── api
│   │   └── mod.rs
│   ├── bin
│   │   ├── nexa-api.rs
│   │   └── nexa.rs
│   ├── cli
│   │   ├── cli_handler.rs
│   │   └── mod.rs
│   ├── config.rs
│   ├── context
│   │   └── mod.rs
│   ├── error
│   │   └── mod.rs
│   ├── lib.rs
│   ├── llm
│   │   ├── mod.rs
│   │   ├── system_helper.rs
│   │   ├── test_utils.rs
│   │   └── tests.rs
│   ├── logging.rs
│   ├── main.rs
│   ├── mcp
│   │   ├── buffer.rs
│   │   ├── cluster
│   │   │   ├── manager.rs
│   │   │   └── types.rs
│   │   ├── cluster.rs
│   │   ├── cluster_processor.rs
│   │   ├── config.rs
│   │   ├── loadbalancer.rs
│   │   ├── metrics.rs
│   │   ├── mod.rs
│   │   ├── processor
│   │   │   └── tests.rs
│   │   ├── processor.rs
│   │   ├── protocol.rs
│   │   ├── registry.rs
│   │   ├── server
│   │   │   ├── config.rs
│   │   │   ├── mod.rs
│   │   │   └── server.rs
│   │   └── tokens.rs
│   ├── memory
│   │   └── mod.rs
│   ├── models
│   │   ├── agent.rs
│   │   └── mod.rs
│   ├── monitoring
│   │   └── mod.rs
│   ├── pipeline
│   │   └── mod.rs
│   ├── server
│   │   └── mod.rs
│   ├── settings.rs
│   ├── startup.rs
│   ├── tokens
│   │   └── mod.rs
│   ├── types
│   │   ├── agent.rs
│   │   ├── cluster.rs
│   │   ├── mod.rs
│   │   └── workflow.rs
│   └── utils.rs
├── test_nexa.sh
├── tests
│   ├── api_test.rs
│   ├── cli_test.rs
│   ├── integration_test.rs
│   ├── llm_api_test.rs
│   ├── startup_test.rs
│   └── stress_test.rs
└── tutorial.md

28 directories, 80 files
