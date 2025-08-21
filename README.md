# 📚 Shelfie - AI File Organizer

Transform messy directories into perfectly organized file systems with AI.

## What it does

- **Smart Analysis**: AI examines file content to create meaningful names and folders
- **Multiple Providers**: OpenAI, Anthropic, or Ollama support
- **Safe Operations**: Shows plan before moving anything
- **All File Types**: Images, PDFs, audio, video, code, archives, and more

## Installation

```bash
# Install from GitHub
cargo install --git https://github.com/natexcvi/shelfie

# Or clone and build
git clone https://github.com/natexcvi/shelfie
cd shelfie
cargo install --path .
```

## Quick Start

```bash
# Set API key
export OPENAI_API_KEY="your_key_here"

# Organize
shelfie /path/to/messy/folder
```

## Example

**Before:**
```
downloads/
├── IMG_001.jpg
├── document.pdf
├── file123.py
└── song.mp3
```

**After:**
```
downloads/
├── images/vacation_sunset_beach.jpg
├── documents/project_specification.pdf
├── scripts/data_processing_script.py
└── media/audio/favorite_song.mp3
```

## Options

```bash
shelfie /path/folder --dry-run    # Preview changes
shelfie /path/folder --show-tree  # Show current structure
```

## Requirements

- Rust
- API key (OpenAI/Anthropic) or Ollama running locally
