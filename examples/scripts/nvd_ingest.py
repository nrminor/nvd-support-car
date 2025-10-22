#!/usr/bin/env python3
# /// script
# requires-python = ">=3.9"
# dependencies = [
#     "httpx>=0.25.0",
#     "pydantic>=2.0.0",
# ]
# ///

"""
NVD Support Car Ingestion Client

This script provides a command-line interface for sending metagenomic analysis results
to the NVD Support Car service. It supports both GOTTCHA2 and STAST data formats and
handles compression and authentication automatically.

The script uses PEP 723 inline script metadata for dependency management, which means
it can be run directly with uv without requiring a separate requirements file or virtual
environment setup.

Usage with uv:
    uv run nvd_ingest.py --help
    uv run nvd_ingest.py gottcha2 --input results.tsv --sample-id SAMPLE001
    uv run nvd_ingest.py stast --input blast_results.tsv --sample-id SAMPLE001 --task megablast

The script can also be used in Nextflow pipelines by placing it in the bin/ directory
and calling it directly from process scripts.
"""

import argparse
import gzip
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

import httpx
from pydantic import BaseModel, ConfigDict, Field


class Gottcha2Record(BaseModel):
    """GOTTCHA2 taxonomic abundance record"""

    model_config = ConfigDict(populate_by_name=True)

    sample_id: str
    level: str
    name: str
    taxid: str
    read_count: int
    total_bp_mapped: int
    ani_ci95: float = Field(alias="ANI_CI95", ge=0, le=1)
    covered_sig_len: int
    best_sig_cov: float = Field(ge=0, le=1)
    depth: float
    rel_abundance: float = Field(ge=0, le=1)


class StastRecord(BaseModel):
    """STAST BLAST hit record"""

    task: str
    sample_id: str
    qseqid: str
    qlen: int
    sseqid: str
    stitle: str
    length: int
    pident: float = Field(ge=0, le=100)
    evalue: float
    bitscore: float
    sscinames: str
    staxids: str
    rank: str


@dataclass
class IngestionClient:
    """Client for NVD Support Car ingestion endpoints"""

    base_url: str
    bearer_token: str
    timeout: float = 30.0
    verify_ssl: bool = True

    def __post_init__(self) -> None:
        """Initialize HTTP client with configured settings"""
        self.client = httpx.Client(
            timeout=self.timeout,
            verify=self.verify_ssl,
            headers={
                "Authorization": f"Bearer {self.bearer_token}",
            },
        )

    def send_records(self, endpoint: str, records: list[BaseModel]) -> bool:
        """
        Send records to the specified endpoint

        The function converts records to JSONL format, compresses with gzip,
        and sends to the server with proper authentication.

        Returns True on success, False on failure
        """
        # Convert records to JSONL
        jsonl_lines = [record.model_dump_json() for record in records]
        jsonl_data = "\n".join(jsonl_lines).encode("utf-8")

        # Compress with gzip
        compressed_data = gzip.compress(jsonl_data)

        # Send request
        try:
            response = self.client.post(
                f"{self.base_url}/{endpoint}",
                content=compressed_data,
                headers={
                    "Content-Type": "application/gzip",
                    "Content-Encoding": "gzip",
                },
            )
            response.raise_for_status()
            print(f"Successfully ingested {len(records)} records to {endpoint}")
            return True

        except httpx.HTTPStatusError as e:
            print(f"HTTP error {e.response.status_code}: {e.response.text}", file=sys.stderr)
            return False
        except Exception as e:
            print(f"Error sending data: {e}", file=sys.stderr)
            return False

    def close(self) -> None:
        """Close the HTTP client connection"""
        self.client.close()


def parse_gottcha2_tsv(filepath: Path, sample_id: str) -> list[Gottcha2Record]:
    """
    Parse GOTTCHA2 TSV output file

    Expected format has columns:
    LEVEL, NAME, TAXID, READ_COUNT, TOTAL_BP_MAPPED, ANI_CI95,
    COVERED_SIG_LEN, BEST_SIG_COV, DEPTH, REL_ABUNDANCE
    """
    records = []

    with open(filepath) as f:
        # Skip header if present
        header = f.readline().strip()
        if not header.startswith("LEVEL"):
            # No header, reset to beginning
            f.seek(0)

        for line_num, line in enumerate(f, start=2):
            parts = line.strip().split("\t")
            if len(parts) < 10:
                print(f"Warning: Skipping line {line_num}, insufficient columns", file=sys.stderr)
                continue

            try:
                record = Gottcha2Record(
                    sample_id=sample_id,
                    level=parts[0],
                    name=parts[1],
                    taxid=parts[2],
                    read_count=int(parts[3]),
                    total_bp_mapped=int(parts[4]),
                    ani_ci95=float(parts[5]),
                    covered_sig_len=int(parts[6]),
                    best_sig_cov=float(parts[7]),
                    depth=float(parts[8]),
                    rel_abundance=float(parts[9]),
                )
                records.append(record)
            except (ValueError, IndexError) as e:
                print(f"Warning: Error parsing line {line_num}: {e}", file=sys.stderr)

    return records


def parse_stast_tsv(filepath: Path, sample_id: str, task: str) -> list[StastRecord]:
    """
    Parse STAST BLAST output file

    Expected format has columns:
    qseqid, qlen, sseqid, stitle, length, pident, evalue, bitscore, sscinames, staxids, rank
    """
    records = []

    with open(filepath) as f:
        # Skip header if present
        header = f.readline().strip()
        if not header.startswith("qseqid"):
            f.seek(0)

        for line_num, line in enumerate(f, start=2):
            parts = line.strip().split("\t")
            if len(parts) < 11:
                print(f"Warning: Skipping line {line_num}, insufficient columns", file=sys.stderr)
                continue

            try:
                record = StastRecord(
                    task=task,
                    sample_id=sample_id,
                    qseqid=parts[0],
                    qlen=int(parts[1]),
                    sseqid=parts[2],
                    stitle=parts[3],
                    length=int(parts[4]),
                    pident=float(parts[5]),
                    evalue=float(parts[6]),
                    bitscore=float(parts[7]),
                    sscinames=parts[8],
                    staxids=parts[9],
                    rank=parts[10],
                )
                records.append(record)
            except (ValueError, IndexError) as e:
                print(f"Warning: Error parsing line {line_num}: {e}", file=sys.stderr)

    return records


def gottcha2_cmd(args: argparse.Namespace, client: IngestionClient) -> None:
    """Ingest GOTTCHA2 taxonomic abundance data"""
    print(f"Parsing GOTTCHA2 data from {args.input}")
    records = parse_gottcha2_tsv(args.input, args.sample_id)

    if not records:
        print("No valid records found", file=sys.stderr)
        sys.exit(1)

    print(f"Found {len(records)} valid records")

    # Send in batches
    for i in range(0, len(records), args.batch_size):
        batch = records[i : i + args.batch_size]
        print(f"Sending batch {i // args.batch_size + 1} ({len(batch)} records)")

        if not client.send_records("ingest-gottcha2", batch):
            print("Failed to send batch", file=sys.stderr)
            sys.exit(1)

    client.close()
    print("All records successfully ingested")


def stast_cmd(args: argparse.Namespace, client: IngestionClient) -> None:
    """Ingest STAST BLAST results"""
    print(f"Parsing STAST data from {args.input}")
    records = parse_stast_tsv(args.input, args.sample_id, args.task)

    if not records:
        print("No valid records found", file=sys.stderr)
        sys.exit(1)

    print(f"Found {len(records)} valid records")

    # Send in batches
    for i in range(0, len(records), args.batch_size):
        batch = records[i : i + args.batch_size]
        print(f"Sending batch {i // args.batch_size + 1} ({len(batch)} records)")

        if not client.send_records("ingest-stast", batch):
            print("Failed to send batch", file=sys.stderr)
            sys.exit(1)

    client.close()
    print("All records successfully ingested")


def healthz_cmd(args: argparse.Namespace, client: IngestionClient) -> None:
    """Check service health"""
    try:
        response = client.client.get(f"{client.base_url}/healthz")
        response.raise_for_status()
        print(f"Service is healthy: {response.text}")
    except Exception as e:
        print(f"Service health check failed: {e}", file=sys.stderr)
        sys.exit(1)
    finally:
        client.close()


def main() -> None:
    """Main CLI entry point"""
    parser = argparse.ArgumentParser(
        description="NVD Support Car ingestion client",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    # Global options
    parser.add_argument(
        "--url",
        default=os.getenv("NVD_SUPPORT_URL", "http://localhost:8080"),
        help="NVD Support Car service URL (env: NVD_SUPPORT_URL)",
    )
    parser.add_argument(
        "--token",
        default=os.getenv("NVD_BEARER_TOKEN"),
        required=not os.getenv("NVD_BEARER_TOKEN"),
        help="Bearer token for authentication (env: NVD_BEARER_TOKEN)",
    )
    parser.add_argument(
        "--no-verify-ssl",
        action="store_true",
        help="Disable SSL certificate verification",
    )

    # Subcommands
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # GOTTCHA2 command
    gottcha2_parser = subparsers.add_parser(
        "gottcha2",
        help="Ingest GOTTCHA2 taxonomic abundance data",
    )
    gottcha2_parser.add_argument(
        "--input",
        "-i",
        type=Path,
        required=True,
        help="GOTTCHA2 TSV file",
    )
    gottcha2_parser.add_argument(
        "--sample-id",
        "-s",
        required=True,
        help="Sample identifier",
    )
    gottcha2_parser.add_argument(
        "--batch-size",
        type=int,
        default=1000,
        help="Records per batch (default: 1000)",
    )

    # STAST command
    stast_parser = subparsers.add_parser(
        "stast",
        help="Ingest STAST BLAST results",
    )
    stast_parser.add_argument(
        "--input",
        "-i",
        type=Path,
        required=True,
        help="STAST BLAST output file",
    )
    stast_parser.add_argument(
        "--sample-id",
        "-s",
        required=True,
        help="Sample identifier",
    )
    stast_parser.add_argument(
        "--task",
        "-t",
        default="megablast",
        help="BLAST task type (e.g., megablast, blastn) (default: megablast)",
    )
    stast_parser.add_argument(
        "--batch-size",
        type=int,
        default=1000,
        help="Records per batch (default: 1000)",
    )

    # Health check command
    healthz_parser = subparsers.add_parser(
        "healthz",
        help="Check service health",
    )

    # Parse arguments
    args = parser.parse_args()

    # Check if command was specified
    if not args.command:
        parser.print_help()
        sys.exit(1)

    # Check required token
    if not args.token:
        print(
            "Error: Bearer token is required (use --token or set NVD_BEARER_TOKEN env var)",
            file=sys.stderr,
        )
        sys.exit(1)

    # Create client
    client = IngestionClient(
        base_url=args.url.rstrip("/"),
        bearer_token=args.token,
        verify_ssl=not args.no_verify_ssl,
    )

    # Execute command
    if args.command == "gottcha2":
        gottcha2_cmd(args, client)
    elif args.command == "stast":
        stast_cmd(args, client)
    elif args.command == "healthz":
        healthz_cmd(args, client)


if __name__ == "__main__":
    main()
