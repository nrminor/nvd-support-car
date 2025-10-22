#!/usr/bin/env Rscript

#' NVD Support Car Ingestion Client (R)
#'
#' This R script provides functionality for sending metagenomic analysis results
#' to the NVD Support Car service. It handles both GOTTCHA2 and STAST data formats,
#' manages authentication, and performs data compression before transmission.
#'
#' The script is designed to work within R-based bioinformatics pipelines and can
#' be integrated into Nextflow workflows by placing it in the bin/ directory. It
#' uses common R packages that are typically available in bioinformatics environments.
#'
#' Required R packages:
#'   - httr: HTTP client for API interactions
#'   - jsonlite: JSON serialization
#'   - optparse: Command-line argument parsing
#'   - dplyr: Data manipulation (optional but recommended)
#'
#' Installation:
#'   install.packages(c("httr", "jsonlite", "optparse"))
#'
#' Usage:
#'   ./nvd_ingest.R --help
#'   ./nvd_ingest.R gottcha2 --input results.tsv --sample-id SAMPLE001
#'   ./nvd_ingest.R stast --input blast.tsv --sample-id SAMPLE001 --task megablast
#'
#' Environment variables:
#'   NVD_SUPPORT_URL: Base URL of the service (default: http://localhost:8080)
#'   NVD_BEARER_TOKEN: Bearer token for authentication (required)

library(httr)
library(jsonlite)
library(optparse)

# Define the ingestion client class
IngestionClient <- setRefClass(
  "IngestionClient",
  fields = list(
    base_url = "character",
    bearer_token = "character",
    timeout = "numeric"
  ),
  methods = list(
    initialize = function(base_url, bearer_token, timeout = 30) {
      base_url <<- sub("/$", "", base_url)
      bearer_token <<- bearer_token
      timeout <<- timeout
    },

    send_records = function(endpoint, records_df) {
      # Convert dataframe to JSONL format
      jsonl_lines <- apply(records_df, 1, function(row) {
        toJSON(as.list(row), auto_unbox = TRUE)
      })
      jsonl_data <- paste(jsonl_lines, collapse = "\n")

      # Compress with gzip
      raw_data <- charToRaw(jsonl_data)
      compressed_data <- memCompress(raw_data, type = "gzip")

      # Prepare the request
      url <- paste0(base_url, "/", endpoint)

      response <- POST(
        url,
        add_headers(
          Authorization = paste("Bearer", bearer_token),
          `Content-Type` = "application/gzip",
          `Content-Encoding` = "gzip"
        ),
        body = compressed_data,
        timeout(timeout)
      )

      if (http_error(response)) {
        cat(
          sprintf(
            "HTTP error %d: %s\n",
            status_code(response),
            content(response, "text")
          ),
          file = stderr()
        )
        return(FALSE)
      }

      cat(sprintf(
        "Successfully ingested %d records to %s\n",
        nrow(records_df),
        endpoint
      ))
      return(TRUE)
    },

    check_health = function() {
      url <- paste0(base_url, "/healthz")

      response <- GET(
        url,
        add_headers(Authorization = paste("Bearer", bearer_token)),
        timeout(timeout)
      )

      if (http_error(response)) {
        cat(
          sprintf(
            "Health check failed with status %d\n",
            status_code(response)
          ),
          file = stderr()
        )
        return(FALSE)
      }

      cat(sprintf("Service is healthy: %s\n", content(response, "text")))
      return(TRUE)
    }
  )
)

# Parse GOTTCHA2 TSV file
parse_gottcha2_tsv <- function(filepath, sample_id) {
  # Read the TSV file
  data <- tryCatch(
    {
      read.table(
        filepath,
        sep = "\t",
        header = TRUE,
        stringsAsFactors = FALSE,
        col.names = c(
          "level",
          "name",
          "taxid",
          "read_count",
          "total_bp_mapped",
          "ani_ci95",
          "covered_sig_len",
          "best_sig_cov",
          "depth",
          "rel_abundance"
        )
      )
    },
    error = function(e) {
      # Try without header
      read.table(
        filepath,
        sep = "\t",
        header = FALSE,
        stringsAsFactors = FALSE,
        col.names = c(
          "level",
          "name",
          "taxid",
          "read_count",
          "total_bp_mapped",
          "ani_ci95",
          "covered_sig_len",
          "best_sig_cov",
          "depth",
          "rel_abundance"
        )
      )
    }
  )

  # Add sample_id column
  data$sample_id <- sample_id

  # Ensure proper data types
  data$read_count <- as.integer(data$read_count)
  data$total_bp_mapped <- as.integer(data$total_bp_mapped)
  data$ani_ci95 <- as.numeric(data$ani_ci95)
  data$covered_sig_len <- as.integer(data$covered_sig_len)
  data$best_sig_cov <- as.numeric(data$best_sig_cov)
  data$depth <- as.numeric(data$depth)
  data$rel_abundance <- as.numeric(data$rel_abundance)

  return(data)
}

# Parse STAST TSV file
parse_stast_tsv <- function(filepath, sample_id, task) {
  # Read the TSV file
  data <- tryCatch(
    {
      read.table(
        filepath,
        sep = "\t",
        header = TRUE,
        stringsAsFactors = FALSE,
        col.names = c(
          "qseqid",
          "qlen",
          "sseqid",
          "stitle",
          "length",
          "pident",
          "evalue",
          "bitscore",
          "sscinames",
          "staxids",
          "rank"
        )
      )
    },
    error = function(e) {
      # Try without header
      read.table(
        filepath,
        sep = "\t",
        header = FALSE,
        stringsAsFactors = FALSE,
        col.names = c(
          "qseqid",
          "qlen",
          "sseqid",
          "stitle",
          "length",
          "pident",
          "evalue",
          "bitscore",
          "sscinames",
          "staxids",
          "rank"
        )
      )
    }
  )

  # Add task and sample_id columns
  data$task <- task
  data$sample_id <- sample_id

  # Ensure proper data types
  data$qlen <- as.integer(data$qlen)
  data$length <- as.integer(data$length)
  data$pident <- as.numeric(data$pident)
  data$evalue <- as.numeric(data$evalue)
  data$bitscore <- as.numeric(data$bitscore)

  return(data)
}

# Process GOTTCHA2 data
process_gottcha2 <- function(client, input_file, sample_id, batch_size = 1000) {
  cat(sprintf("Parsing GOTTCHA2 data from %s\n", input_file))

  records <- parse_gottcha2_tsv(input_file, sample_id)

  if (nrow(records) == 0) {
    cat("No valid records found\n", file = stderr())
    quit(status = 1)
  }

  cat(sprintf("Found %d valid records\n", nrow(records)))

  # Send in batches
  num_batches <- ceiling(nrow(records) / batch_size)
  for (i in 1:num_batches) {
    start_idx <- (i - 1) * batch_size + 1
    end_idx <- min(i * batch_size, nrow(records))
    batch <- records[start_idx:end_idx, ]

    cat(sprintf("Sending batch %d (%d records)\n", i, nrow(batch)))

    if (!client$send_records("ingest-gottcha2", batch)) {
      cat("Failed to send batch\n", file = stderr())
      quit(status = 1)
    }
  }

  cat("All records successfully ingested\n")
}

# Process STAST data
process_stast <- function(
  client,
  input_file,
  sample_id,
  task,
  batch_size = 1000
) {
  cat(sprintf("Parsing STAST data from %s\n", input_file))

  records <- parse_stast_tsv(input_file, sample_id, task)

  if (nrow(records) == 0) {
    cat("No valid records found\n", file = stderr())
    quit(status = 1)
  }

  cat(sprintf("Found %d valid records\n", nrow(records)))

  # Send in batches
  num_batches <- ceiling(nrow(records) / batch_size)
  for (i in 1:num_batches) {
    start_idx <- (i - 1) * batch_size + 1
    end_idx <- min(i * batch_size, nrow(records))
    batch <- records[start_idx:end_idx, ]

    cat(sprintf("Sending batch %d (%d records)\n", i, nrow(batch)))

    if (!client$send_records("ingest-stast", batch)) {
      cat("Failed to send batch\n", file = stderr())
      quit(status = 1)
    }
  }

  cat("All records successfully ingested\n")
}

# Main function
main <- function() {
  # Define command line options
  option_list <- list(
    make_option(
      c("-u", "--url"),
      type = "character",
      default = Sys.getenv("NVD_SUPPORT_URL", "http://localhost:8080"),
      help = "NVD Support Car service URL [default %default]"
    ),
    make_option(
      c("-t", "--token"),
      type = "character",
      default = Sys.getenv("NVD_BEARER_TOKEN"),
      help = "Bearer token for authentication [env: NVD_BEARER_TOKEN]"
    ),
    make_option(
      c("-i", "--input"),
      type = "character",
      help = "Input TSV file"
    ),
    make_option(
      c("-s", "--sample-id"),
      type = "character",
      help = "Sample identifier"
    ),
    make_option(
      c("--task"),
      type = "character",
      default = "megablast",
      help = "BLAST task type (for stast) [default %default]"
    ),
    make_option(
      c("-b", "--batch-size"),
      type = "integer",
      default = 1000,
      help = "Records per batch [default %default]"
    )
  )

  # Parse arguments
  parser <- OptionParser(
    usage = "%prog [command] [options]\n\nCommands:\n  gottcha2  Ingest GOTTCHA2 data\n  stast     Ingest STAST data\n  health    Check service health",
    option_list = option_list
  )

  args <- commandArgs(trailingOnly = TRUE)

  if (length(args) == 0) {
    print_help(parser)
    quit(status = 0)
  }

  command <- args[1]
  opt <- parse_args(parser, args = args[-1])

  # Check for required token
  if (is.null(opt$token) || opt$token == "") {
    cat(
      "Error: Bearer token is required (--token or NVD_BEARER_TOKEN env var)\n",
      file = stderr()
    )
    quit(status = 1)
  }

  # Create client
  client <- IngestionClient$new(
    base_url = opt$url,
    bearer_token = opt$token
  )

  # Execute command
  if (command == "gottcha2") {
    if (is.null(opt$input) || is.null(opt$`sample-id`)) {
      cat(
        "Error: --input and --sample-id are required for gottcha2 command\n",
        file = stderr()
      )
      quit(status = 1)
    }

    if (!file.exists(opt$input)) {
      cat(
        sprintf("Error: Input file not found: %s\n", opt$input),
        file = stderr()
      )
      quit(status = 1)
    }

    process_gottcha2(client, opt$input, opt$`sample-id`, opt$`batch-size`)
  } else if (command == "stast") {
    if (is.null(opt$input) || is.null(opt$`sample-id`)) {
      cat(
        "Error: --input and --sample-id are required for stast command\n",
        file = stderr()
      )
      quit(status = 1)
    }

    if (!file.exists(opt$input)) {
      cat(
        sprintf("Error: Input file not found: %s\n", opt$input),
        file = stderr()
      )
      quit(status = 1)
    }

    process_stast(
      client,
      opt$input,
      opt$`sample-id`,
      opt$task,
      opt$`batch-size`
    )
  } else if (command == "health") {
    client$check_health()
  } else {
    cat(sprintf("Error: Unknown command '%s'\n", command), file = stderr())
    print_help(parser)
    quit(status = 1)
  }
}

# Run if executed directly
if (!interactive()) {
  main()
}
