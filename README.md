# 🤖 FS Organiser - AI-Powered File Organization Tool

A Rust CLI application that uses AI to automatically organize your files by analyzing their content and creating logical directory structures.

## Features

- **Content Analysis**: Uses LLM to analyze file content and determine meaningful names
- **Smart Organization**: Creates logical directory structures based on file relationships
- **Multiple File Types**: Supports text, images, PDFs, and code files
- **User Approval**: Shows reorganization plan before making changes
- **OpenAI Integration**: Uses OpenAI models for intelligent file analysis

## Prerequisites

- Rust (latest stable version)
- OpenAI API key

## Installation

1. Clone and build:
```bash
git clone <repository-url>
cd fs-organiser
cargo build --release
```

2. Set up your OpenAI API key:
```bash
export OPENAI_API_KEY="your_api_key_here"
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

1. **Scan Directory**: Recursively scans the target directory for files
2. **Analyze Filenames**: Determines if filenames are descriptive enough
3. **Content Analysis**: For non-descriptive files, analyzes content to suggest better names
4. **Structure Planning**: Creates an optimal directory organization plan
5. **User Approval**: Shows the plan and asks for confirmation
6. **Execution**: Moves files to their new locations with better names

## Supported File Types

- **Text Files**: `.txt`, `.md`, `.rs`, `.py`, `.js`, `.json`, etc.
- **Images**: `.png`, `.jpg`, `.jpeg`, `.gif`, `.bmp`, `.webp`
- **PDFs**: `.pdf` (with text extraction)
- **Code Files**: Most programming language extensions

## Example

Input directory:
```
messy_folder/
├── IMG_001.jpg
├── document.pdf
├── data.txt
└── file123.py
```

After organization:
```
organized_folder/
├── photos/
│   └── sunset_beach_vacation.jpg
├── documents/
│   └── project_specification.pdf
├── data/
│   └── customer_contact_list.txt
└── scripts/
    └── data_processing_script.py
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