# NVD Support Car Client Scripts

This directory contains scripts in a few common languages that can act as
clients for interacting with the NVD Support Car service. Each script can be
used on its own given the required dependencies (see below) and are meant to
demonstrate how a Nextflow or Snakemake pipeline might parallelize networked API
calls on an HPC cluster. As an example, any one of these scripts can be placed
in a Nextflow `bin/` directory, made executable, and used as-is. All other
configuration takes place through environment variables.

## Overview

The NVD Support Car service accepts gzipped JSONL data containing metagenomic
analysis results from GOTTCHA2 or NVD's STAT+BLAST ("STAST") subworkflow. Each
client script handles the conversion from standard TSV formats produced by
GOTTCHA2 and STAST tools to the required compressed
[NDJSON/JSONL](https://jsonltools.com/jsonl-vs-ndjson) format. Upon successful
conversion, they also handle authentication transmission to the support car.

Each script provides the same core functionality but is tailored for different
runtime environments and dependency requirements. Choose the script that best
fits your computational environment and existing toolchain.

## Available Scripts

### Python Client (nvd_ingest.py)

The Python client uses modern Python features including PEP 723 inline script
metadata for dependency management.
[This allows the script to be run with uv](https://docs.astral.sh/uv/guides/scripts/#declaring-script-dependencies)
without requiring manual dependency installation or virtual environment
management. The script provides a subcommand-based command-line interface using
Python's argparse module and handles HTTP communication with
[httpx](https://www.python-httpx.org/).

The Python client is ideal for environments where Python is the primary language
for data processing and analysis. It integrates well with other Python-based
bioinformatics tools and can be easily extended with additional functionality.
The script also uses [Pydantic](https://docs.pydantic.dev/latest/) for data
validation and serialization to JSON and thus ensures that records conform to
the expected schema before transmission.

To use the Python client with uv, simply run it directly and uv will handle all
dependencies automatically. The script includes comprehensive error handling and
progress reporting, making it suitable for both interactive use and automated
pipelines.

### TypeScript/Bun Client (nvd_ingest.ts)

The TypeScript client is designed to run with [Bun](https://bun.com/), a fast
JavaScript runtime that includes built-in TypeScript support. Bun provides
excellent startup performance and eliminates the need for a separate compilation
step, making it well-suited for use in pipeline environments where scripts are
invoked frequently.

This client leverages TypeScript's type system to ensure data consistency and
provides clear interfaces for the GOTTCHA2 and STAST record formats. The script
uses Node.js-compatible APIs for file handling and stream processing, allowing
it to efficiently process large result files without loading everything into
memory.

The TypeScript client is particularly useful in environments where
JavaScript/TypeScript is already in use, or where the fast startup time of Bun
is advantageous. It requires only Bun to be installed, with no additional
dependencies needed at runtime. And if that's too much to ask, it can also be
[compiled into standalone dependency-free executables](https://bun.com/docs/bundler/executables)
with `bun build --compile`

### R Client (nvd_ingest.R)

We also provide an R client for folks partial to more R-based bioinformatics
workflows. It uses standard R packages that are commonly available in
bioinformatics computing environments: httr for HTTP communication, jsonlite for
JSON handling, and optparse for command-line argument parsing.

The script follows R conventions and uses reference classes to encapsulate the
client functionality. It handles data frames naturally, making it easy to
integrate with other R-based data processing steps. The batch processing
functionality ensures that large datasets can be transmitted without memory
issues.

Use the R client if you're a researcher already working in R for your
statistical analyses and want to directly upload results without switching to
another language. Like the other scripts here, the R client can be sourced from
other R scripts or run as a standalone command-line tool.

### Shell Client (nvd_ingest.sh)

The shell client is the most portable option, requiring only standard Unix
tools: bash, curl, gzip, and awk. It has no external dependencies beyond what is
typically available on any Unix-like system, making it ideal for restrictive HPC
environments or minimal container images.

Despite using only shell scripting, this client provides full functionality
including TSV to JSONL conversion, batch processing, and comprehensive error
handling. The script uses awk for efficient text processing and can handle large
files without loading them entirely into memory.

The shell client is particularly valuable in environments where installing
additional software is difficult or prohibited. It can be used directly in
Nextflow pipelines without any additional setup and provides colored output for
better readability when run interactively.

## Installation and Setup

### For Nextflow Pipelines

To use these scripts in a Nextflow pipeline, place them in the `bin/` directory
of your pipeline repository. Nextflow will automatically make them available to
all processes. Ensure the scripts are executable:

```bash
chmod +x bin/nvd_ingest.py
chmod +x bin/nvd_ingest.ts
chmod +x bin/nvd_ingest.R
chmod +x bin/nvd_ingest.sh
```

Then use them in your process definitions:

```nextflow
process ingestGottcha2 {
    input:
    path gottcha2_results
    val sample_id

    script:
    """
    nvd_ingest.py gottcha2 \
        --input ${gottcha2_results} \
        --sample-id ${sample_id}
    """
}
```

### Environment Configuration

All scripts support configuration through environment variables, which is
particularly useful in pipeline contexts:

- `NVD_SUPPORT_URL`: The base URL of the NVD Support Car service
- `NVD_BEARER_TOKEN`: The authentication token for the service

These can be set in your Nextflow configuration:

```nextflow
env {
    NVD_SUPPORT_URL = 'https://nvd.your-domain.com'
    NVD_BEARER_TOKEN = secrets.nvd_token
}
```

### Dependency Installation

For the Python client with uv:

```bash
# No installation needed - just run with uv
uv run nvd_ingest.py --help
```

For the TypeScript client with Bun:

```bash
# Install Bun if not already installed
curl -fsSL https://bun.sh/install | bash

# Run the script
bun run nvd_ingest.ts --help
```

For the R client:

```R
# Install required packages if not already available
install.packages(c("httr", "jsonlite", "optparse"))
```

The shell client requires no additional installation beyond ensuring curl is
available.

## Usage Examples

All scripts follow the same command-line interface pattern for consistency. Here
are examples using each client:

### Processing GOTTCHA2 Results

```bash
# Python
uv run nvd_ingest.py gottcha2 \
    --input gottcha2_output.tsv \
    --sample-id SAMPLE_001 \
    --batch-size 5000

# TypeScript/Bun
bun run nvd_ingest.ts gottcha2 \
    --input gottcha2_output.tsv \
    --sample-id SAMPLE_001 \
    --batch-size 5000

# R
./nvd_ingest.R gottcha2 \
    --input gottcha2_output.tsv \
    --sample-id SAMPLE_001 \
    --batch-size 5000

# Shell
./nvd_ingest.sh gottcha2 \
    --input gottcha2_output.tsv \
    --sample-id SAMPLE_001 \
    --batch-size 5000
```

### Processing STAST BLAST Results

```bash
# Python
uv run nvd_ingest.py stast \
    --input blast_results.tsv \
    --sample-id SAMPLE_001 \
    --task megablast

# TypeScript/Bun
bun run nvd_ingest.ts stast \
    --input blast_results.tsv \
    --sample-id SAMPLE_001 \
    --task blastn

# R
./nvd_ingest.R stast \
    --input blast_results.tsv \
    --sample-id SAMPLE_001 \
    --task megablast

# Shell
./nvd_ingest.sh stast \
    --input blast_results.tsv \
    --sample-id SAMPLE_001 \
    --task megablast
```

### Health Check

All scripts support a health check command to verify service connectivity:

```bash
# Python
uv run nvd_ingest.py healthz

# TypeScript/Bun
bun run nvd_ingest.ts health

# R
./nvd_ingest.R health

# Shell
./nvd_ingest.sh health
```

## Data Format Expectations

### GOTTCHA2 TSV Format

The scripts expect GOTTCHA2 output with the following tab-separated columns:

1. LEVEL - Taxonomic level (e.g., phylum, genus, species)
2. NAME - Taxonomic name
3. TAXID - NCBI taxonomy ID
4. READ_COUNT - Number of reads assigned
5. TOTAL_BP_MAPPED - Total base pairs mapped
6. ANI_CI95 - Average nucleotide identity confidence interval
7. COVERED_SIG_LEN - Length of covered signature
8. BEST_SIG_COV - Best signature coverage
9. DEPTH - Coverage depth
10. REL_ABUNDANCE - Relative abundance

### STAST TSV Format

The scripts expect STAST BLAST output with the following tab-separated columns:

1. qseqid - Query sequence ID
2. qlen - Query sequence length
3. sseqid - Subject sequence ID
4. stitle - Subject title
5. length - Alignment length
6. pident - Percentage identity
7. evalue - E-value
8. bitscore - Bit score
9. sscinames - Subject scientific names
10. staxids - Subject taxonomy IDs
11. rank - Taxonomic rank

## Batch Processing

All scripts support batch processing to handle large datasets efficiently. The
batch size can be configured with the `--batch-size` parameter. Records are
processed in chunks to avoid memory issues and provide progress feedback. If a
batch fails to transmit, the script will report the error and exit, allowing for
troubleshooting before retrying.

The default batch size of 1000 records provides a good balance between
efficiency and memory usage. For very large files or constrained environments,
you may want to reduce the batch size. For high-bandwidth connections and
powerful systems, increasing the batch size can improve throughput.

## Error Handling

Each script provides comprehensive error handling and reporting:

- Input file validation ensures files exist before processing
- TSV parsing errors are reported with line numbers
- HTTP errors include status codes and server responses
- Network timeouts and connection issues are caught and reported
- Invalid data that cannot be parsed is skipped with warnings

In pipeline contexts, all scripts use appropriate exit codes to signal success
or failure, allowing Nextflow or other workflow managers to handle retries or
alternative processing paths.

## Security Considerations

The bearer token used for authentication should be treated as a sensitive
credential. In production environments:

- Store tokens in secure credential management systems
- Use Nextflow secrets or similar mechanisms for token injection
- Avoid hardcoding tokens in scripts or configuration files
- Rotate tokens regularly according to your security policies
- Use TLS/HTTPS endpoints for all production deployments

The scripts do not store or log authentication tokens, and they support SSL
certificate verification (which can be disabled for development environments if
necessary).

## Performance Considerations

Performance characteristics vary by implementation:

The shell script has minimal startup overhead and memory usage, making it ideal
for processing many small files. The Python and R clients have higher startup
costs but provide better performance for large files due to optimized JSON
serialization. The TypeScript/Bun client offers the best balance with fast
startup and good processing speed.

For very large files (gigabytes), consider splitting them before processing or
using the shell script with its streaming approach. All scripts process files in
a streaming fashion where possible to minimize memory usage.

## Troubleshooting

Common issues and solutions:

If you encounter authentication errors, verify that your bearer token is
correctly set and has not expired. Check that the service URL is correct and
accessible from your network. For SSL/TLS errors in development, you may need to
disable certificate verification (not recommended for production).

For parsing errors, ensure your TSV files match the expected format. Headers are
optional but if present, should match the expected column names. The scripts
will skip malformed lines with warnings rather than failing completely.

For network timeouts, check your connection to the service and consider reducing
the batch size for slow connections. The scripts use reasonable default timeouts
but these can be adjusted if needed by modifying the source code.

## Integration with Nextflow

These scripts are designed to work seamlessly with Nextflow pipelines. They
follow Nextflow conventions:

- Exit with non-zero codes on failure
- Write progress information to stderr
- Support environment variable configuration
- Can be placed in bin/ directory for automatic availability

Example Nextflow process:

```nextflow
process uploadResults {
    tag "$sample_id"
    
    input:
    tuple val(sample_id), path(gottcha2_results), path(stast_results)
    
    script:
    """
    # Upload GOTTCHA2 results
    nvd_ingest.sh gottcha2 \
        --input ${gottcha2_results} \
        --sample-id ${sample_id}
    
    # Upload STAST results
    nvd_ingest.sh stast \
        --input ${stast_results} \
        --sample-id ${sample_id} \
        --task megablast
    """
}
```

## Contributing

These scripts are starting points that can be extended and customized for
specific needs. When modifying the scripts:

- Maintain consistency in command-line interfaces across all implementations
- Preserve the batch processing capability for large datasets
- Ensure error messages are informative and actionable
- Test with both small and large input files
- Verify compatibility with Nextflow execution environment

Consider contributing improvements back to the repository to benefit other users
of the NVD Support Car service.
