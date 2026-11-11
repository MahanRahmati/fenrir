# Providers

## Open Router

### Configuration

```toml
[general]
model = "open_router/nvidia/nemotron-3-ultra-550b-a55b:free"

[[providers.open_router.models]]
name = "nvidia/nemotron-3-ultra-550b-a55b:free"
alias = "NVIDIA Nemotron 3 Ultra 550B (550B-A55B)"
```

### Secrets

```toml
open_router_key = ""
```

## OpenCode

### Configuration

```toml
[general]
model = "opencode/deepseek-v4.1-flash"

[[providers.opencode.models]]
name = "deepseek-v4.1-flash"
alias = "DeepSeek V4.1 Flash"
```

### Secrets

```toml
opencode_key = ""
```

## Z.AI

### Configuration

```toml
[general]
model = "zai/glm-4.7-flash"

[[providers.zai.models]]
name = "glm-4.7-flash"
alias = "GLM 4.7 Flash"
```

### Secrets

```toml
zai_key = ""
```
