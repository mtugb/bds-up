# bds-up
A tool that update your minecraft bedrock dedicated server.

## Features
- **Automated Update**: Fetches the latest stable/preview versions automatically.
- **Config Preservation**: ``server.properties`` will be updated into latest format while keeping your existing setting.
- **Data Migration**: Automatically copy your ``worlds/``, ``allowlists``, and others necessary.
- **Backup Supported**: Before updating, you'll be asked if you want to make backup. (backup will be created in sibling dir of server root dir)
- **Executable Permission**: This tool will give server dir excutable permission after updating so that the bds server can get started just by executing official command, ``LD_LIBRARY_PATH=. ./bedrock_server``.

## Supported OS
As this tool is designed for linux environment which is commonly used for Minecraft Server, this tool only support Ubuntu / Linux and is **not compatible with Windows or other OS**.

## Installation
```
# Fetch binary and put on eligible place
curl -L https://github.com/mtugb/bds-up/releases/latest/download/bdsup -o bdsup
chmod +x bdsup
mkdir -p ~/bin
mv -f bdsup ~/bin/
# If necessary, add one required line to ~/.bashrc
if ! grep -q '~/bin' ~/.bashrc; then
  echo 'export PATH="$HOME/bin:$PATH"' >> ~/.bashrc
  echo "Path added to .bashrc. Please restart your shell or run 'source ~/.bashrc'."
fi
```

## Usage
```bash
cd my_bedrock_server/
# Interective updated (this update BDS server in . (current directory))
bdsup
```
### Options
```bash
# Specify a custom target directory
bdsup --target-dir /path/to/server

# Force update to stable/preview without interaction
bdsup --stable
bdsup --preview
```

## Credits & Respects
- [Bedrock-OSS/BDS-Versions](https://github.com/Bedrock-OSS/BDS-Versions)
- [StackOverFlow: Copy Dir All](https://stackoverflow.com/a/65192210)


