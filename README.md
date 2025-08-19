# 🤖 FS Organiser - AI-Powered File Organization Tool

A Rust CLI application that uses AI to automatically organize your files by analyzing their content and creating logical directory structures.

## Features

- **Content Analysis**: Uses LLM to analyze file content and determine meaningful names
- **Smart Organization**: Creates logical directory structures based on file relationships
- **Robust File Detection**: Uses content-based analysis (via `infer` crate) rather than just extensions
- **Multiple File Types**: Supports text, images, PDFs, audio, video, and archive files
- **User Approval**: Shows reorganization plan before making changes
- **Multiple AI Providers**: Supports OpenAI, Anthropic, and Ollama models
- **Concurrent Processing**: Analyzes up to 10 files simultaneously with progress bars
- **Model Filtering**: Filter large model lists by typing (e.g., "gpt-4" or "claude-3")

## Prerequisites

- Rust (latest stable version)
- At least one of:
  - OpenAI API key
  - Anthropic API key  
  - Ollama running locally

## Installation

1. Clone and build:
```bash
git clone <repository-url>
cd fs-organiser
cargo build --release
```

2. Set up your API keys (choose one or more):
```bash
# For OpenAI
export OPENAI_API_KEY="your_openai_key_here"

# For Anthropic
export ANTHROPIC_API_KEY="your_anthropic_key_here"

# For Ollama (make sure it's running)
ollama serve
ollama pull llama2  # or any other model
```

## Usage

### Basic Usage
```bash
# Organize a directory
cargo run /path/to/messy/directory

# Or use the built binary
./target/release/fs-organiser /path/to/messy/directory
```

### Options
```bash
# Show current directory tree before organizing
cargo run -- /path/to/directory --show-tree

# Show what would be done without making changes
cargo run -- /path/to/directory --dry-run
```

## How It Works

1. **Provider Selection**: Choose between OpenAI, Anthropic, or Ollama
2. **Model Selection**: Pick from available models with optional filtering
3. **Scan Directory**: Recursively scans the target directory for files
4. **Concurrent Analysis**: Analyzes filenames in parallel (up to 10 at once) with progress bars
5. **Content Analysis**: For non-descriptive files, generates better names concurrently
6. **Structure Planning**: Creates an optimal directory organization plan
7. **User Approval**: Shows the plan and asks for confirmation
8. **Execution**: Moves files to their new locations with better names

## Supported File Types

The application uses the `infer` crate for robust, content-based file type detection:

- **Text Files**: `.txt`, `.md`, `.rs`, `.py`, `.js`, `.json`, etc.
- **Images**: PNG, JPEG, GIF, BMP, WebP, TIFF, and more (detected by content)
- **PDFs**: PDF documents with text extraction
- **Audio**: MP3, WAV, FLAC, OGG, and other audio formats (detected by content)
- **Video**: MP4, AVI, MOV, WebM, and other video formats (detected by content)
- **Archives**: ZIP, RAR, TAR, GZIP, 7Z, and other archives (detected by content)
- **Code Files**: Most programming language extensions

## Example

Input directory:
```
messy_folder/
├── IMG_001.jpg        # Will be detected as JPEG image
├── document.pdf       # Will be detected as PDF 
├── data.txt          # Will be detected as text file
├── file123.py        # Will be detected as text file
├── song.mp3          # Will be detected as MP3 audio
├── video.mkv         # Will be detected as video file
└── archive.zip       # Will be detected as ZIP archive
```

After organization:
```
organized_folder/
├── images/
│   └── sunset_beach_vacation.jpg
├── documents/
│   └── project_specification.pdf
├── data/
│   └── customer_contact_list.txt
├── scripts/
│   └── data_processing_script.py
├── media/
│   ├── audio/
│   │   └── favorite_song.mp3
│   └── video/
│       └── presentation_recording.mkv
└── archives/
    └── backup_files.zip
```

## Configuration

The tool automatically:
- Detects file types by extension
- Uses GPT-4 for analysis (you can modify the model in provider selection)
- Creates backup plans before moving files
- Handles file conflicts gracefully

## Limitations

- Requires OpenAI API key (costs apply for API usage)
- Works best with files that have readable content
- Binary files (except images/PDFs) are skipped
- Large directories may take time to process

## License

MIT License - see LICENSE file for details.