#!/usr/bin/env bun

/**
 * NVD Support Car Ingestion Client (TypeScript/Bun)
 *
 * This TypeScript client provides functionality for ingesting metagenomic analysis
 * results into the NVD Support Car service. It is designed to run with Bun, which
 * provides fast startup times and built-in TypeScript support without requiring
 * compilation.
 *
 * The script handles GOTTCHA2 and STAST data formats, manages authentication via
 * bearer tokens, and automatically compresses data using gzip before transmission.
 * It can process large result files in batches to avoid memory issues and provides
 * progress feedback during ingestion.
 *
 * Installation of Bun:
 *   curl -fsSL https://bun.sh/install | bash
 *
 * Usage:
 *   bun run nvd_ingest.ts --help
 *   bun run nvd_ingest.ts gottcha2 --input results.tsv --sample-id SAMPLE001
 *   bun run nvd_ingest.ts stast --input blast.tsv --sample-id SAMPLE001 --task megablast
 *
 * Environment variables:
 *   NVD_SUPPORT_URL: Base URL of the NVD Support Car service
 *   NVD_BEARER_TOKEN: Bearer token for authentication
 *
 * When used in Nextflow pipelines, this script can be placed in the bin/ directory
 * and will be automatically available to all processes. The script's shebang line
 * ensures it will be executed with Bun when called directly.
 */

import { createReadStream, existsSync } from "node:fs";
import process from "node:process";
import readline from "node:readline";

// Type definitions for our data structures
interface Gottcha2Record {
	sample_id: string;
	level: string;
	name: string;
	taxid: string;
	read_count: number;
	total_bp_mapped: number;
	ani_ci95: number;
	covered_sig_len: number;
	best_sig_cov: number;
	depth: number;
	rel_abundance: number;
}

interface StastRecord {
	task: string;
	sample_id: string;
	qseqid: string;
	qlen: number;
	sseqid: string;
	stitle: string;
	length: number;
	pident: number;
	evalue: number;
	bitscore: number;
	sscinames: string;
	staxids: string;
	rank: string;
}

class IngestionClient {
	private baseUrl: string;
	private bearerToken: string;
	private headers: Record<string, string>;

	constructor(baseUrl: string, bearerToken: string) {
		this.baseUrl = baseUrl.replace(/\/$/, "");
		this.bearerToken = bearerToken;
		this.headers = {
			Authorization: `Bearer ${bearerToken}`,
			"Content-Type": "application/gzip",
			"Content-Encoding": "gzip",
		};
	}

	async sendRecords<T>(endpoint: string, records: T[]): Promise<boolean> {
		// Convert records to JSONL
		const jsonlData = records
			.map((record) => JSON.stringify(record))
			.join("\n");

		// Compress with gzip
		const encoder = new TextEncoder();
		const uncompressed = encoder.encode(jsonlData);
		const compressed = Bun.gzipSync(uncompressed);

		try {
			const response = await fetch(`${this.baseUrl}/${endpoint}`, {
				method: "POST",
				headers: this.headers,
				body: compressed,
			});

			if (!response.ok) {
				const errorText = await response.text();
				console.error(`HTTP error ${response.status}: ${errorText}`);
				return false;
			}

			console.log(
				`Successfully ingested ${records.length} records to ${endpoint}`,
			);
			return true;
		} catch (error) {
			console.error(`Error sending data: ${error}`);
			return false;
		}
	}

	async checkHealth(): Promise<boolean> {
		try {
			const response = await fetch(`${this.baseUrl}/healthz`, {
				headers: {
					Authorization: `Bearer ${this.bearerToken}`,
				},
			});

			if (response.ok) {
				const text = await response.text();
				console.log(`Service is healthy: ${text}`);
				return true;
			} else {
				console.error(`Health check failed with status ${response.status}`);
				return false;
			}
		} catch (error) {
			console.error(`Health check error: ${error}`);
			return false;
		}
	}
}

async function parseGottcha2TSV(
	filepath: string,
	sampleId: string,
): Promise<Gottcha2Record[]> {
	const records: Gottcha2Record[] = [];
	const fileStream = createReadStream(filepath);
	const rl = readline.createInterface({
		input: fileStream,
		crlfDelay: Infinity,
	});

	let lineNum = 0;
	for await (const line of rl) {
		lineNum++;

		// Skip header
		if (lineNum === 1 && line.startsWith("LEVEL")) {
			continue;
		}

		const parts = line.trim().split("\t");
		if (parts.length < 10) {
			console.warn(`Warning: Skipping line ${lineNum}, insufficient columns`);
			continue;
		}

		try {
			const record: Gottcha2Record = {
				sample_id: sampleId,
				level: parts[0],
				name: parts[1],
				taxid: parts[2],
				read_count: parseInt(parts[3], 10),
				total_bp_mapped: parseInt(parts[4], 10),
				ani_ci95: parseFloat(parts[5]),
				covered_sig_len: parseInt(parts[6], 10),
				best_sig_cov: parseFloat(parts[7]),
				depth: parseFloat(parts[8]),
				rel_abundance: parseFloat(parts[9]),
			};
			records.push(record);
		} catch (error) {
			console.warn(`Warning: Error parsing line ${lineNum}: ${error}`);
		}
	}

	return records;
}

async function parseStastTSV(
	filepath: string,
	sampleId: string,
	task: string,
): Promise<StastRecord[]> {
	const records: StastRecord[] = [];
	const fileStream = createReadStream(filepath);
	const rl = readline.createInterface({
		input: fileStream,
		crlfDelay: Infinity,
	});

	let lineNum = 0;
	for await (const line of rl) {
		lineNum++;

		// Skip header
		if (lineNum === 1 && line.startsWith("qseqid")) {
			continue;
		}

		const parts = line.trim().split("\t");
		if (parts.length < 11) {
			console.warn(`Warning: Skipping line ${lineNum}, insufficient columns`);
			continue;
		}

		try {
			const record: StastRecord = {
				task: task,
				sample_id: sampleId,
				qseqid: parts[0],
				qlen: parseInt(parts[1], 10),
				sseqid: parts[2],
				stitle: parts[3],
				length: parseInt(parts[4], 10),
				pident: parseFloat(parts[5]),
				evalue: parseFloat(parts[6]),
				bitscore: parseFloat(parts[7]),
				sscinames: parts[8],
				staxids: parts[9],
				rank: parts[10],
			};
			records.push(record);
		} catch (error) {
			console.warn(`Warning: Error parsing line ${lineNum}: ${error}`);
		}
	}

	return records;
}

async function ingestGottcha2(
	client: IngestionClient,
	inputFile: string,
	sampleId: string,
	batchSize: number = 1000,
): Promise<void> {
	console.log(`Parsing GOTTCHA2 data from ${inputFile}`);
	const records = await parseGottcha2TSV(inputFile, sampleId);

	if (records.length === 0) {
		console.error("No valid records found");
		process.exit(1);
	}

	console.log(`Found ${records.length} valid records`);

	// Send in batches
	for (let i = 0; i < records.length; i += batchSize) {
		const batch = records.slice(i, i + batchSize);
		const batchNum = Math.floor(i / batchSize) + 1;
		console.log(`Sending batch ${batchNum} (${batch.length} records)`);

		if (!(await client.sendRecords("ingest-gottcha2", batch))) {
			console.error("Failed to send batch");
			process.exit(1);
		}
	}

	console.log("All records successfully ingested");
}

async function ingestStast(
	client: IngestionClient,
	inputFile: string,
	sampleId: string,
	task: string,
	batchSize: number = 1000,
): Promise<void> {
	console.log(`Parsing STAST data from ${inputFile}`);
	const records = await parseStastTSV(inputFile, sampleId, task);

	if (records.length === 0) {
		console.error("No valid records found");
		process.exit(1);
	}

	console.log(`Found ${records.length} valid records`);

	// Send in batches
	for (let i = 0; i < records.length; i += batchSize) {
		const batch = records.slice(i, i + batchSize);
		const batchNum = Math.floor(i / batchSize) + 1;
		console.log(`Sending batch ${batchNum} (${batch.length} records)`);

		if (!(await client.sendRecords("ingest-stast", batch))) {
			console.error("Failed to send batch");
			process.exit(1);
		}
	}

	console.log("All records successfully ingested");
}

function printUsage() {
	console.log(`
NVD Support Car Ingestion Client

Usage:
  bun run nvd_ingest.ts <command> [options]

Commands:
  gottcha2    Ingest GOTTCHA2 taxonomic abundance data
  stast       Ingest STAST BLAST results
  health      Check service health

Options:
  --url <url>         NVD Support Car service URL (env: NVD_SUPPORT_URL)
  --token <token>     Bearer token for authentication (env: NVD_BEARER_TOKEN)
  --input <file>      Input TSV file
  --sample-id <id>    Sample identifier
  --task <task>       BLAST task type (for stast command, default: megablast)
  --batch-size <n>    Records per batch (default: 1000)
  --help              Show this help message

Examples:
  bun run nvd_ingest.ts gottcha2 --input results.tsv --sample-id SAMPLE001
  bun run nvd_ingest.ts stast --input blast.tsv --sample-id SAMPLE001 --task blastn
  bun run nvd_ingest.ts health
`);
}

// Main entry point
async function main() {
	const args = process.argv.slice(2);

	if (args.length === 0 || args.includes("--help")) {
		printUsage();
		process.exit(0);
	}

	const command = args[0];

	// Parse command line arguments
	const getArg = (name: string, defaultValue?: string): string | undefined => {
		const index = args.indexOf(`--${name}`);
		if (index !== -1 && index + 1 < args.length) {
			return args[index + 1];
		}
		return defaultValue;
	};

	// Get configuration from environment or command line
	const url =
		getArg("url") || process.env.NVD_SUPPORT_URL || "http://localhost:8080";
	const token = getArg("token") || process.env.NVD_BEARER_TOKEN;

	if (!token) {
		console.error(
			"Error: Bearer token is required (--token or NVD_BEARER_TOKEN env var)",
		);
		process.exit(1);
	}

	const client = new IngestionClient(url, token);

	switch (command) {
		case "gottcha2": {
			const inputFile = getArg("input");
			const sampleId = getArg("sample-id");
			const batchSize = parseInt(getArg("batch-size", "1000") || "1000", 10);

			if (!inputFile || !sampleId) {
				console.error(
					"Error: --input and --sample-id are required for gottcha2 command",
				);
				process.exit(1);
			}

			if (!existsSync(inputFile)) {
				console.error(`Error: Input file not found: ${inputFile}`);
				process.exit(1);
			}

			// biome-ignore lint/style/noNonNullAssertion: <Just a script--it's fine if these are null here anyway>
			await ingestGottcha2(client, inputFile!, sampleId!, batchSize);
			break;
		}

		case "stast": {
			const inputFile = getArg("input");
			const sampleId = getArg("sample-id");
			const task = getArg("task", "megablast") || "megablast";
			const batchSize = parseInt(getArg("batch-size", "1000") || "1000", 10);

			if (!inputFile || !sampleId) {
				console.error(
					"Error: --input and --sample-id are required for stast command",
				);
				process.exit(1);
			}

			if (!existsSync(inputFile)) {
				console.error(`Error: Input file not found: ${inputFile}`);
				process.exit(1);
			}

			// biome-ignore lint/style/noNonNullAssertion: <Just a script--it's fine if these are null here anyway>
			await ingestStast(client, inputFile!, sampleId!, task, batchSize);
			break;
		}

		case "health": {
			await client.checkHealth();
			break;
		}

		default:
			console.error(`Error: Unknown command '${command}'`);
			printUsage();
			process.exit(1);
	}
}

// Run the main function
main().catch((error) => {
	console.error("Fatal error:", error);
	process.exit(1);
});
