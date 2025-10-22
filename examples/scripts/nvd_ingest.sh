#!/usr/bin/env bash

# NVD Support Car Ingestion Client (Shell/curl)
#
# This shell script provides a lightweight client for ingesting metagenomic analysis
# results into the NVD Support Car service using only standard Unix tools. It requires
# no external dependencies beyond curl, gzip, and jq (optional but recommended for
# JSON formatting).
#
# The script is particularly useful in environments where installing additional software
# is difficult or undesirable, such as HPC clusters or containerized environments. It
# can be directly integrated into Nextflow pipelines by placing it in the bin/ directory.
#
# Dependencies:
#   - curl: HTTP client (required)
#   - gzip: Compression utility (required)
#   - jq: JSON processor (optional, for better error messages)
#   - awk: Text processing (required, standard on all Unix systems)
#
# Usage:
#   ./nvd_ingest.sh --help
#   ./nvd_ingest.sh gottcha2 --input results.tsv --sample-id SAMPLE001
#   ./nvd_ingest.sh stast --input blast.tsv --sample-id SAMPLE001 --task megablast
#   ./nvd_ingest.sh health
#
# Environment variables:
#   NVD_SUPPORT_URL: Base URL of the service (default: http://localhost:8080)
#   NVD_BEARER_TOKEN: Bearer token for authentication (required)

set -euo pipefail

# Configuration defaults
readonly DEFAULT_URL="http://localhost:8080"
readonly DEFAULT_BATCH_SIZE=1000
readonly DEFAULT_TASK="megablast"
readonly SCRIPT_NAME=$(basename "$0")

# Color output for better readability (disabled in non-TTY environments)
if [[ -t 1 ]]; then
	readonly RED='\033[0;31m'
	readonly GREEN='\033[0;32m'
	readonly YELLOW='\033[1;33m'
	readonly NC='\033[0m' # No Color
else
	readonly RED=''
	readonly GREEN=''
	readonly YELLOW=''
	readonly NC=''
fi

# Logging functions
log_info() {
	echo -e "${GREEN}[INFO]${NC} $*" >&2
}

log_warn() {
	echo -e "${YELLOW}[WARN]${NC} $*" >&2
}

log_error() {
	echo -e "${RED}[ERROR]${NC} $*" >&2
}

# Print usage information
print_usage() {
	cat <<EOF
NVD Support Car Ingestion Client

Usage:
  $SCRIPT_NAME <command> [options]

Commands:
  gottcha2    Ingest GOTTCHA2 taxonomic abundance data
  stast       Ingest STAST BLAST results
  health      Check service health

Global Options:
  --url <url>         NVD Support Car service URL
                      (env: NVD_SUPPORT_URL, default: $DEFAULT_URL)
  --token <token>     Bearer token for authentication
                      (env: NVD_BEARER_TOKEN, required)
  --help              Show this help message

Command-specific Options:
  --input <file>      Input TSV file (required for gottcha2/stast)
  --sample-id <id>    Sample identifier (required for gottcha2/stast)
  --task <task>       BLAST task type (for stast, default: $DEFAULT_TASK)
  --batch-size <n>    Records per batch (default: $DEFAULT_BATCH_SIZE)

Examples:
  # Ingest GOTTCHA2 results
  $SCRIPT_NAME gottcha2 --input results.tsv --sample-id SAMPLE001

  # Ingest STAST BLAST results
  $SCRIPT_NAME stast --input blast.tsv --sample-id SAMPLE001 --task blastn

  # Check service health
  $SCRIPT_NAME health

  # Using environment variables
  export NVD_SUPPORT_URL="https://nvd.example.com"
  export NVD_BEARER_TOKEN="your-secret-token"
  $SCRIPT_NAME gottcha2 --input results.tsv --sample-id SAMPLE001

EOF
}

# Parse command line arguments
parse_args() {
	local command=""
	local url="${NVD_SUPPORT_URL:-$DEFAULT_URL}"
	local token="${NVD_BEARER_TOKEN:-}"
	local input=""
	local sample_id=""
	local task="$DEFAULT_TASK"
	local batch_size="$DEFAULT_BATCH_SIZE"

	# Check if any arguments provided
	if [[ $# -eq 0 ]]; then
		print_usage
		exit 0
	fi

	# Get command
	command="$1"
	shift

	# Parse remaining arguments
	while [[ $# -gt 0 ]]; do
		case "$1" in
		--help | -h)
			print_usage
			exit 0
			;;
		--url)
			url="$2"
			shift 2
			;;
		--token)
			token="$2"
			shift 2
			;;
		--input | -i)
			input="$2"
			shift 2
			;;
		--sample-id | -s)
			sample_id="$2"
			shift 2
			;;
		--task | -t)
			task="$2"
			shift 2
			;;
		--batch-size | -b)
			batch_size="$2"
			shift 2
			;;
		*)
			log_error "Unknown option: $1"
			print_usage
			exit 1
			;;
		esac
	done

	# Export parsed values for use in functions
	export COMMAND="$command"
	export BASE_URL="${url%/}" # Remove trailing slash
	export BEARER_TOKEN="$token"
	export INPUT_FILE="$input"
	export SAMPLE_ID="$sample_id"
	export TASK="$task"
	export BATCH_SIZE="$batch_size"
}

# Validate required parameters
validate_params() {
	if [[ -z "$BEARER_TOKEN" ]]; then
		log_error "Bearer token is required (--token or NVD_BEARER_TOKEN env var)"
		exit 1
	fi

	case "$COMMAND" in
	gottcha2 | stast)
		if [[ -z "$INPUT_FILE" ]]; then
			log_error "--input is required for $COMMAND command"
			exit 1
		fi
		if [[ ! -f "$INPUT_FILE" ]]; then
			log_error "Input file not found: $INPUT_FILE"
			exit 1
		fi
		if [[ -z "$SAMPLE_ID" ]]; then
			log_error "--sample-id is required for $COMMAND command"
			exit 1
		fi
		;;
	esac
}

# Convert TSV to JSONL for GOTTCHA2
convert_gottcha2_to_jsonl() {
	local input_file="$1"
	local sample_id="$2"
	local output_file="$3"

	awk -F'\t' -v sample_id="$sample_id" '
    BEGIN {
        OFS = ""
    }
    NR == 1 && $1 ~ /^LEVEL/ {
        # Skip header
        next
    }
    NF >= 10 {
        # Escape special characters in strings
        gsub(/"/, "\\\"", $2)  # name
        gsub(/"/, "\\\"", $3)  # taxid
        
        print "{",
            "\"sample_id\":\"", sample_id, "\",",
            "\"level\":\"", $1, "\",",
            "\"name\":\"", $2, "\",",
            "\"taxid\":\"", $3, "\",",
            "\"read_count\":", $4, ",",
            "\"total_bp_mapped\":", $5, ",",
            "\"ani_ci95\":", $6, ",",
            "\"covered_sig_len\":", $7, ",",
            "\"best_sig_cov\":", $8, ",",
            "\"depth\":", $9, ",",
            "\"rel_abundance\":", $10,
            "}"
    }
    ' "$input_file" >"$output_file"
}

# Convert TSV to JSONL for STAST
convert_stast_to_jsonl() {
	local input_file="$1"
	local sample_id="$2"
	local task="$3"
	local output_file="$4"

	awk -F'\t' -v sample_id="$sample_id" -v task="$task" '
    BEGIN {
        OFS = ""
    }
    NR == 1 && $1 ~ /^qseqid/ {
        # Skip header
        next
    }
    NF >= 11 {
        # Escape special characters in strings
        gsub(/"/, "\\\"", $1)  # qseqid
        gsub(/"/, "\\\"", $3)  # sseqid
        gsub(/"/, "\\\"", $4)  # stitle
        gsub(/"/, "\\\"", $9)  # sscinames
        gsub(/"/, "\\\"", $10) # staxids
        gsub(/"/, "\\\"", $11) # rank
        
        print "{",
            "\"task\":\"", task, "\",",
            "\"sample_id\":\"", sample_id, "\",",
            "\"qseqid\":\"", $1, "\",",
            "\"qlen\":", $2, ",",
            "\"sseqid\":\"", $3, "\",",
            "\"stitle\":\"", $4, "\",",
            "\"length\":", $5, ",",
            "\"pident\":", $6, ",",
            "\"evalue\":", $7, ",",
            "\"bitscore\":", $8, ",",
            "\"sscinames\":\"", $9, "\",",
            "\"staxids\":\"", $10, "\",",
            "\"rank\":\"", $11, "\"",
            "}"
    }
    ' "$input_file" >"$output_file"
}

# Send data to the server
send_data() {
	local endpoint="$1"
	local jsonl_file="$2"
	local batch_size="$3"

	# Create temporary file for gzipped data
	local temp_gz=$(mktemp)
	trap "rm -f $temp_gz" EXIT

	# Count total records
	local total_records=$(wc -l <"$jsonl_file")
	log_info "Found $total_records records to send"

	if [[ $total_records -eq 0 ]]; then
		log_error "No valid records found in input file"
		exit 1
	fi

	# Process in batches
	local batch_num=0
	local start_line=1

	while [[ $start_line -le $total_records ]]; do
		batch_num=$((batch_num + 1))
		local end_line=$((start_line + batch_size - 1))
		if [[ $end_line -gt $total_records ]]; then
			end_line=$total_records
		fi

		local batch_count=$((end_line - start_line + 1))
		log_info "Sending batch $batch_num ($batch_count records)"

		# Extract batch and compress
		sed -n "${start_line},${end_line}p" "$jsonl_file" | gzip >"$temp_gz"

		# Send request
		local response
		local http_code

		response=$(curl -s -w "\n%{http_code}" \
			-X POST \
			-H "Authorization: Bearer $BEARER_TOKEN" \
			-H "Content-Type: application/gzip" \
			-H "Content-Encoding: gzip" \
			--data-binary "@$temp_gz" \
			"$BASE_URL/$endpoint" 2>&1) || true

		http_code=$(echo "$response" | tail -n1)
		response_body=$(echo "$response" | head -n-1)

		if [[ "$http_code" != "200" ]]; then
			log_error "Failed to send batch $batch_num (HTTP $http_code)"
			if [[ -n "$response_body" ]]; then
				log_error "Server response: $response_body"
			fi
			exit 1
		fi

		log_info "Batch $batch_num successfully sent"
		start_line=$((end_line + 1))
	done

	log_info "All records successfully ingested"
}

# Process GOTTCHA2 data
process_gottcha2() {
	log_info "Processing GOTTCHA2 data from $INPUT_FILE"

	# Create temporary file for JSONL
	local temp_jsonl=$(mktemp)
	trap "rm -f $temp_jsonl" EXIT

	# Convert TSV to JSONL
	convert_gottcha2_to_jsonl "$INPUT_FILE" "$SAMPLE_ID" "$temp_jsonl"

	# Send data
	send_data "ingest-gottcha2" "$temp_jsonl" "$BATCH_SIZE"
}

# Process STAST data
process_stast() {
	log_info "Processing STAST data from $INPUT_FILE"

	# Create temporary file for JSONL
	local temp_jsonl=$(mktemp)
	trap "rm -f $temp_jsonl" EXIT

	# Convert TSV to JSONL
	convert_stast_to_jsonl "$INPUT_FILE" "$SAMPLE_ID" "$TASK" "$temp_jsonl"

	# Send data
	send_data "ingest-stast" "$temp_jsonl" "$BATCH_SIZE"
}

# Check service health
check_health() {
	log_info "Checking service health at $BASE_URL"

	local response
	local http_code

	response=$(curl -s -w "\n%{http_code}" \
		-H "Authorization: Bearer $BEARER_TOKEN" \
		"$BASE_URL/healthz" 2>&1) || true

	http_code=$(echo "$response" | tail -n1)
	response_body=$(echo "$response" | head -n-1)

	if [[ "$http_code" == "200" ]]; then
		log_info "Service is healthy: $response_body"
	else
		log_error "Health check failed (HTTP $http_code)"
		if [[ -n "$response_body" ]]; then
			log_error "Server response: $response_body"
		fi
		exit 1
	fi
}

# Main execution
main() {
	parse_args "$@"
	validate_params

	case "$COMMAND" in
	gottcha2)
		process_gottcha2
		;;
	stast)
		process_stast
		;;
	health)
		check_health
		;;
	*)
		log_error "Unknown command: $COMMAND"
		print_usage
		exit 1
		;;
	esac
}

# Run main function
main "$@"
