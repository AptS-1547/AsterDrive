import type { Language } from "prism-react-renderer";
import { themes } from "prism-react-renderer";
import Prism from "prismjs/components/prism-core.js";

const prismGlobal = globalThis as typeof globalThis & { Prism?: typeof Prism };

prismGlobal.Prism = Prism;

const PRISM_COMPONENT_LOADERS = {
	bash: () => import("prismjs/components/prism-bash.js"),
	batch: () => import("prismjs/components/prism-batch.js"),
	c: () => import("prismjs/components/prism-c.js"),
	clike: () => import("prismjs/components/prism-clike.js"),
	clojure: () => import("prismjs/components/prism-clojure.js"),
	coffeescript: () => import("prismjs/components/prism-coffeescript.js"),
	cpp: () => import("prismjs/components/prism-cpp.js"),
	csharp: () => import("prismjs/components/prism-csharp.js"),
	css: () => import("prismjs/components/prism-css.js"),
	dart: () => import("prismjs/components/prism-dart.js"),
	docker: () => import("prismjs/components/prism-docker.js"),
	elixir: () => import("prismjs/components/prism-elixir.js"),
	go: () => import("prismjs/components/prism-go.js"),
	graphql: () => import("prismjs/components/prism-graphql.js"),
	groovy: () => import("prismjs/components/prism-groovy.js"),
	hcl: () => import("prismjs/components/prism-hcl.js"),
	ini: () => import("prismjs/components/prism-ini.js"),
	java: () => import("prismjs/components/prism-java.js"),
	javascript: () => import("prismjs/components/prism-javascript.js"),
	json: () => import("prismjs/components/prism-json.js"),
	julia: () => import("prismjs/components/prism-julia.js"),
	kotlin: () => import("prismjs/components/prism-kotlin.js"),
	less: () => import("prismjs/components/prism-less.js"),
	lua: () => import("prismjs/components/prism-lua.js"),
	markdown: () => import("prismjs/components/prism-markdown.js"),
	markup: () => import("prismjs/components/prism-markup.js"),
	"markup-templating": () =>
		import("prismjs/components/prism-markup-templating.js"),
	perl: () => import("prismjs/components/prism-perl.js"),
	php: () => import("prismjs/components/prism-php.js"),
	powershell: () => import("prismjs/components/prism-powershell.js"),
	protobuf: () => import("prismjs/components/prism-protobuf.js"),
	python: () => import("prismjs/components/prism-python.js"),
	r: () => import("prismjs/components/prism-r.js"),
	rest: () => import("prismjs/components/prism-rest.js"),
	ruby: () => import("prismjs/components/prism-ruby.js"),
	rust: () => import("prismjs/components/prism-rust.js"),
	scala: () => import("prismjs/components/prism-scala.js"),
	scss: () => import("prismjs/components/prism-scss.js"),
	solidity: () => import("prismjs/components/prism-solidity.js"),
	sql: () => import("prismjs/components/prism-sql.js"),
	swift: () => import("prismjs/components/prism-swift.js"),
	toml: () => import("prismjs/components/prism-toml.js"),
	typescript: () => import("prismjs/components/prism-typescript.js"),
	verilog: () => import("prismjs/components/prism-verilog.js"),
	yaml: () => import("prismjs/components/prism-yaml.js"),
} as const;

type PrismComponentId = keyof typeof PRISM_COMPONENT_LOADERS;

const PRISM_COMPONENT_DEPENDENCIES: Record<
	PrismComponentId,
	PrismComponentId[]
> = {
	bash: [],
	batch: [],
	c: ["clike"],
	clike: [],
	clojure: [],
	coffeescript: ["javascript"],
	cpp: ["c"],
	csharp: ["clike"],
	css: [],
	dart: ["clike"],
	docker: [],
	elixir: [],
	go: ["clike"],
	graphql: [],
	groovy: ["clike"],
	hcl: [],
	ini: [],
	java: ["clike"],
	javascript: ["clike"],
	json: [],
	julia: [],
	kotlin: ["clike"],
	less: ["css"],
	lua: [],
	markdown: ["markup"],
	markup: [],
	"markup-templating": ["markup"],
	perl: [],
	php: ["markup-templating"],
	powershell: [],
	protobuf: ["clike"],
	python: [],
	r: [],
	rest: [],
	ruby: ["clike"],
	rust: [],
	scala: ["java"],
	scss: ["css"],
	solidity: ["clike"],
	sql: [],
	swift: [],
	toml: [],
	typescript: ["javascript"],
	verilog: [],
	yaml: [],
};

type PrismLanguageConfig = {
	components: PrismComponentId[];
	grammar: Language;
};

const FALLBACK_PRISM_LANGUAGE: PrismLanguageConfig = {
	grammar: "text",
	components: [],
};

const PRISM_LANGUAGE_MAP: Record<string, PrismLanguageConfig> = {
	bat: { grammar: "batch", components: ["batch"] },
	c: { grammar: "c", components: ["c"] },
	clojure: { grammar: "clojure", components: ["clojure"] },
	coffeescript: { grammar: "coffeescript", components: ["coffeescript"] },
	cpp: { grammar: "cpp", components: ["cpp"] },
	csharp: { grammar: "csharp", components: ["csharp"] },
	css: { grammar: "css", components: ["css"] },
	dart: { grammar: "dart", components: ["dart"] },
	dockerfile: { grammar: "docker", components: ["docker"] },
	elixir: { grammar: "elixir", components: ["elixir"] },
	go: { grammar: "go", components: ["go"] },
	graphql: { grammar: "graphql", components: ["graphql"] },
	groovy: { grammar: "groovy", components: ["groovy"] },
	hcl: { grammar: "hcl", components: ["hcl"] },
	html: { grammar: "markup", components: ["markup"] },
	ini: { grammar: "ini", components: ["ini"] },
	java: { grammar: "java", components: ["java"] },
	javascript: { grammar: "javascript", components: ["javascript"] },
	json: { grammar: "json", components: ["json"] },
	julia: { grammar: "julia", components: ["julia"] },
	kotlin: { grammar: "kotlin", components: ["kotlin"] },
	less: { grammar: "less", components: ["less"] },
	lua: { grammar: "lua", components: ["lua"] },
	markdown: { grammar: "markdown", components: ["markdown"] },
	perl: { grammar: "perl", components: ["perl"] },
	php: { grammar: "php", components: ["php"] },
	plaintext: FALLBACK_PRISM_LANGUAGE,
	powershell: { grammar: "powershell", components: ["powershell"] },
	protobuf: { grammar: "protobuf", components: ["protobuf"] },
	python: { grammar: "python", components: ["python"] },
	r: { grammar: "r", components: ["r"] },
	restructuredtext: { grammar: "rest", components: ["rest"] },
	ruby: { grammar: "ruby", components: ["ruby"] },
	rust: { grammar: "rust", components: ["rust"] },
	scala: { grammar: "scala", components: ["scala"] },
	scss: { grammar: "scss", components: ["scss"] },
	shell: { grammar: "bash", components: ["bash"] },
	sol: { grammar: "solidity", components: ["solidity"] },
	sql: { grammar: "sql", components: ["sql"] },
	swift: { grammar: "swift", components: ["swift"] },
	systemverilog: { grammar: "verilog", components: ["verilog"] },
	toml: { grammar: "toml", components: ["toml"] },
	typescript: { grammar: "typescript", components: ["typescript"] },
	verilog: { grammar: "verilog", components: ["verilog"] },
	xml: { grammar: "markup", components: ["markup"] },
	yaml: { grammar: "yaml", components: ["yaml"] },
};

const prismComponentLoads = new Map<PrismComponentId, Promise<void>>();

export { Prism };

export function createEditorPalette(theme: string) {
	if (theme === "vs-dark") {
		return {
			background: "#1e1e1e",
			border: "#2a2a2a",
			caret: "#ffffff",
			gutterBackground: "#181818",
			gutterForeground: "#858585",
			selection: "rgba(38, 79, 120, 0.35)",
			theme: themes.vsDark,
		};
	}

	return {
		background: "#ffffff",
		border: "#d0d7de",
		caret: "#1f2328",
		gutterBackground: "#f6f8fa",
		gutterForeground: "#6e7781",
		selection: "rgba(9, 105, 218, 0.20)",
		theme: themes.vsLight,
	};
}

export function normalizePrismLanguage(language: string): Language {
	return PRISM_LANGUAGE_MAP[language]?.grammar ?? "text";
}

export function getPrismLanguageConfig(language: string): PrismLanguageConfig {
	return PRISM_LANGUAGE_MAP[language] ?? FALLBACK_PRISM_LANGUAGE;
}

export function hasPrismGrammar(language: Language) {
	return language === "text" || language in Prism.languages;
}

function ensurePrismComponent(component: PrismComponentId): Promise<void> {
	const pendingLoad = prismComponentLoads.get(component);

	if (pendingLoad) {
		return pendingLoad;
	}

	const loadPromise: Promise<void> = Promise.all(
		PRISM_COMPONENT_DEPENDENCIES[component].map(ensurePrismComponent),
	)
		.then(async () => {
			if (component in Prism.languages) {
				return;
			}

			await PRISM_COMPONENT_LOADERS[component]();
		})
		.catch((error) => {
			prismComponentLoads.delete(component);
			throw error;
		});

	prismComponentLoads.set(component, loadPromise);

	return loadPromise;
}

export function ensurePrismLanguage(
	config: PrismLanguageConfig,
): Promise<void> {
	return Promise.all(config.components.map(ensurePrismComponent)).then(
		() => undefined,
	);
}
