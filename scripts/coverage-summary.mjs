import fs from "node:fs";
import path from "node:path";

function usage() {
	console.error("Usage: bun scripts/coverage-summary.mjs <label> <lcov-path>");
	process.exit(1);
}

function formatPercent(covered, total) {
	if (total === 0) return "N/A";
	return `${((covered / total) * 100).toFixed(2)}%`;
}

function parseLcov(filePath) {
	const content = fs.readFileSync(filePath, "utf8");
	const metrics = {
		lines: { covered: 0, total: 0 },
		functions: { covered: 0, total: 0 },
		branches: { covered: 0, total: 0 },
	};

	for (const line of content.split(/\r?\n/)) {
		if (line.startsWith("LH:")) {
			metrics.lines.covered += Number.parseInt(line.slice(3), 10);
		} else if (line.startsWith("LF:")) {
			metrics.lines.total += Number.parseInt(line.slice(3), 10);
		} else if (line.startsWith("FNH:")) {
			metrics.functions.covered += Number.parseInt(line.slice(4), 10);
		} else if (line.startsWith("FNF:")) {
			metrics.functions.total += Number.parseInt(line.slice(4), 10);
		} else if (line.startsWith("BRH:")) {
			metrics.branches.covered += Number.parseInt(line.slice(4), 10);
		} else if (line.startsWith("BRF:")) {
			metrics.branches.total += Number.parseInt(line.slice(4), 10);
		}
	}

	return metrics;
}

function buildSummary(label, lcovPath, metrics) {
	const rows = [
		["Lines", metrics.lines.covered, metrics.lines.total],
		["Functions", metrics.functions.covered, metrics.functions.total],
	];

	if (metrics.branches.total > 0) {
		rows.push(["Branches", metrics.branches.covered, metrics.branches.total]);
	}

	const tableRows = rows
		.map(
			([metric, covered, total]) =>
				`| ${metric} | ${formatPercent(covered, total)} | ${covered}/${total} |`,
		)
		.join("\n");

	return [
		`## ${label} Coverage`,
		"",
		`Source: \`${path.normalize(lcovPath)}\``,
		"",
		"| Metric | Coverage | Hit/Total |",
		"| --- | --- | --- |",
		tableRows,
		"",
	].join("\n");
}

const [label, lcovPath] = process.argv.slice(2);

if (!label || !lcovPath) {
	usage();
}

const metrics = parseLcov(lcovPath);
const summary = buildSummary(label, lcovPath, metrics);
const stepSummaryPath = process.env.GITHUB_STEP_SUMMARY;

if (stepSummaryPath) {
	fs.appendFileSync(stepSummaryPath, `${summary}\n`);
}

console.log(summary);
