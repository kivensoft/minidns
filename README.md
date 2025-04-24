# minidns
mini static and dymamic dns server

## 功能特性

- **DNS标准**：支持标准的DNS查询协议，可作为DNS服务使用，当本地查询不到时会自动转发到上级DNS服务器。
- **动态DNS**：允许用户通过自定义的动态dns协议注册和修改域名解析记录。
- **超低内存占用**：运行只占用几百KB内存。
- **超低CPU占用**：使用单线程、IO轮询模式，实现并发查询下的超低CPU占用。


## 安装指南

### 安装步骤

1. **克隆项目**：
   ```bash
   git clone https://github.com/kivensoft/minidns.git
   cd minidns
   cargo build --release
