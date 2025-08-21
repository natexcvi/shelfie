# 📚 Shelfie - AI File Organizer

Transform messy directories into perfectly organized file systems with AI.

## What it does

- **Smart Analysis**: AI examines file content to create meaningful names and folders
- **Multiple Providers**: OpenAI, Anthropic, or Ollama support
- **Safe Operations**: Shows plan before moving anything
- **All File Types**: Images, PDFs, audio, video, code, archives, and more

> ⚠️ **Privacy Warning**: When using external LLM providers (OpenAI, Anthropic), previews of the contents of all files in the target directory will be sent to the LLM service for analysis. Only use with files you're comfortable sharing. For sensitive data, consider using Ollama with a local model instead.

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
├── IMG_20240315_142853.jpg
├── Screenshot_2024-03-15.png
├── bank_statement.pdf
├── insurance_doc.pdf
├── tax_form_2023.pdf
├── tax_receipt_medical.pdf
├── tax_w2_form.pdf
├── meeting_notes.py
├── project_analysis.ipynb
├── flight_booking.pdf
├── hotel_confirmation.pdf
├── vacation_itinerary.txt
└── passport_photo.jpg
```

**After:**
```
downloads/
├── life-admin/
│   ├── personal-documents/
│   │   ├── monthly_bank_statement.pdf
│   │   └── health_insurance_policy.pdf
│   └── taxes/
│       ├── annual_tax_return_2023.pdf
│       ├── medical_expenses_receipt.pdf
│       └── employer_w2_tax_document.pdf
├── work/
│   ├── scripts/
│   │   └── team_meeting_automation.py
│   └── analysis/
│       └── quarterly_sales_analysis.ipynb
└── travel/
    ├── bookings/
    │   ├── airline_flight_reservation.pdf
    │   └── hotel_booking_confirmation.pdf
    ├── planning/
    │   └── europe_vacation_itinerary.txt
    └── documents/
        └── passport_renewal_photo.jpg
```

## Requirements

- Rust
- API key (OpenAI/Anthropic) or Ollama running locally
