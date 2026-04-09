import {
	Highlight,
	type Token as HighlightToken,
	type Language,
	type RenderProps,
	themes,
} from "prism-react-renderer";
import Prism from "prismjs/components/prism-core.js";
import {
	type KeyboardEvent,
	useDeferredValue,
	useEffect,
	useRef,
	useState,
} from "react";
import { withScopedPrismClassName } from "./prismClassNames";

const KEY_CODE = {
	KeyS: 49,
} as const;

const KEY_MOD = {
	CtrlCmd: 2048,
} as const;

const MONO_FONT_FAMILY =
	'"SFMono-Regular", "SF Mono", "Cascadia Code", "Fira Code", Consolas, "Liberation Mono", Menlo, monospace';

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
	grammar: Language;
	components: PrismComponentId[];
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

type EditorCommandHandler = () => void;

type EditorLike = {
	addCommand: (keybinding: number, handler: EditorCommandHandler) => void;
};

type EditorShortcutApi = {
	KeyCode: typeof KEY_CODE;
	KeyMod: typeof KEY_MOD;
};

export type CodePreviewEditorMountHandler = (
	editor: EditorLike,
	shortcutApi: EditorShortcutApi,
) => void;

interface CodePreviewEditorProps {
	language: string;
	theme: string;
	value: string;
	onChange?: (value: string) => void;
	onMount?: CodePreviewEditorMountHandler;
	options?: {
		domReadOnly?: boolean;
		fontSize?: number;
		lineNumbers?: "on" | "off";
		minimap?: {
			enabled?: boolean;
		};
		padding?: {
			top?: number;
		};
		readOnly?: boolean;
		renderLineHighlight?: "line" | "none";
		scrollBeyondLastLine?: boolean;
		wordWrap?: "on" | "off";
	};
}

function normalizePrismLanguage(language: string): Language {
	return PRISM_LANGUAGE_MAP[language]?.grammar ?? "text";
}

function getPrismLanguageConfig(language: string): PrismLanguageConfig {
	return PRISM_LANGUAGE_MAP[language] ?? FALLBACK_PRISM_LANGUAGE;
}

function hasPrismGrammar(language: Language) {
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

function ensurePrismLanguage(config: PrismLanguageConfig): Promise<void> {
	return Promise.all(config.components.map(ensurePrismComponent)).then(
		() => undefined,
	);
}

function createEditorPalette(theme: string) {
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

function getKeybindingFromEvent(event: KeyboardEvent<HTMLTextAreaElement>) {
	let keybinding = 0;

	if (event.metaKey || event.ctrlKey) {
		keybinding |= KEY_MOD.CtrlCmd;
	}

	if (event.key.toLowerCase() === "s") {
		keybinding |= KEY_CODE.KeyS;
	}

	return keybinding;
}

function insertTabAtSelection(textarea: HTMLTextAreaElement) {
	const { selectionEnd, selectionStart } = textarea;

	textarea.setRangeText("\t", selectionStart, selectionEnd, "end");

	return textarea.value;
}

function renderLineNumbers(lineCount: number, lineHeight: number) {
	return Array.from({ length: lineCount }, (_, index) => String(index + 1)).map(
		(lineNumber) => (
			<div
				key={lineNumber}
				style={{
					height: lineHeight,
					lineHeight: `${lineHeight}px`,
				}}
			>
				{lineNumber}
			</div>
		),
	);
}

function getHighlightTokenSignature(token: HighlightToken) {
	return `${token.types.join(".")}::${token.content}`;
}

function getHighlightLineSignature(line: HighlightToken[]) {
	const signature = line.map(getHighlightTokenSignature).join("\u0001");

	return signature || "empty-line";
}

function renderHighlightedLines({
	tokens,
	getLineProps,
	getTokenProps,
}: Pick<RenderProps, "tokens" | "getLineProps" | "getTokenProps">) {
	const lineOccurrences = new Map<string, number>();

	return tokens.map((line) => {
		const lineSignature = getHighlightLineSignature(line);
		const lineOccurrence = lineOccurrences.get(lineSignature) ?? 0;
		const lineKey = `${lineSignature}::${lineOccurrence}`;
		const tokenOccurrences = new Map<string, number>();

		lineOccurrences.set(lineSignature, lineOccurrence + 1);

		return (
			<div
				key={lineKey}
				{...withScopedPrismClassName(getLineProps({ line, key: lineKey }))}
			>
				{line.map((token) => {
					const tokenSignature = getHighlightTokenSignature(token);
					const tokenOccurrence = tokenOccurrences.get(tokenSignature) ?? 0;
					const tokenKey = `${lineKey}::${tokenSignature}::${tokenOccurrence}`;

					tokenOccurrences.set(tokenSignature, tokenOccurrence + 1);

					return (
						<span
							key={tokenKey}
							{...withScopedPrismClassName(
								getTokenProps({ token, key: tokenKey }),
							)}
						/>
					);
				})}
			</div>
		);
	});
}

export function CodePreviewEditor({
	language,
	theme,
	value,
	onChange,
	onMount,
	options,
}: CodePreviewEditorProps) {
	const [, setPrismRevision] = useState(0);
	const commandsRef = useRef(new Map<number, EditorCommandHandler>());
	const gutterContentRef = useRef<HTMLDivElement | null>(null);
	const onMountRef = useRef(onMount);
	const overlayContentRef = useRef<HTMLDivElement | null>(null);
	const textareaRef = useRef<HTMLTextAreaElement | null>(null);

	const deferredValue = useDeferredValue(value);
	const palette = createEditorPalette(theme);
	const prismConfig = getPrismLanguageConfig(language);
	const prismLanguage = normalizePrismLanguage(language);
	const resolvedPrismLanguage = hasPrismGrammar(prismLanguage)
		? prismLanguage
		: "text";
	const readOnly = options?.readOnly ?? options?.domReadOnly ?? false;
	const fontSize = options?.fontSize ?? 13;
	const lineHeight = Math.round(fontSize * 1.85);
	const lineCount = value.split("\n").length;
	const showLineNumbers = options?.lineNumbers !== "off";
	const gutterWidth = showLineNumbers
		? Math.max(44, String(lineCount).length * 10 + 20)
		: 0;
	const paddingTop = options?.padding?.top ?? 0;
	const paddingBottom =
		options?.scrollBeyondLastLine === false ? 16 : lineHeight * 4;
	const prismValue = readOnly ? value : deferredValue;
	const wrap = options?.wordWrap !== "off";

	useEffect(() => {
		onMountRef.current = onMount;
	}, [onMount]);

	useEffect(() => {
		let cancelled = false;

		if (hasPrismGrammar(prismConfig.grammar)) {
			return;
		}

		void ensurePrismLanguage(prismConfig).then(() => {
			if (!cancelled) {
				setPrismRevision((revision) => revision + 1);
			}
		});

		return () => {
			cancelled = true;
		};
	}, [prismConfig]);

	useEffect(() => {
		commandsRef.current.clear();
		onMountRef.current?.(
			{
				addCommand(keybinding, handler) {
					commandsRef.current.set(keybinding, handler);
				},
			},
			{
				KeyCode: KEY_CODE,
				KeyMod: KEY_MOD,
			},
		);

		return () => {
			commandsRef.current.clear();
		};
	}, []);

	return (
		<div
			className="h-full w-full overflow-hidden"
			style={{ background: palette.background }}
		>
			{readOnly ? (
				<div className="h-full overflow-auto">
					<div
						className="grid min-h-full min-w-full"
						style={{
							gridTemplateColumns: showLineNumbers
								? `${gutterWidth}px minmax(0, 1fr)`
								: "minmax(0, 1fr)",
						}}
					>
						{showLineNumbers ? (
							<div
								className="border-r px-2 text-right select-none"
								style={{
									background: palette.gutterBackground,
									borderColor: palette.border,
									color: palette.gutterForeground,
									fontFamily: MONO_FONT_FAMILY,
									fontSize,
									paddingTop,
								}}
							>
								{renderLineNumbers(lineCount, lineHeight)}
							</div>
						) : null}
						<div className="min-w-0">
							<Highlight
								prism={Prism}
								theme={palette.theme}
								code={prismValue}
								language={resolvedPrismLanguage}
							>
								{({
									className,
									style,
									tokens,
									getLineProps,
									getTokenProps,
								}) => (
									<pre
										className={className}
										style={{
											...style,
											background: "transparent",
											fontFamily: MONO_FONT_FAMILY,
											fontSize,
											lineHeight: `${lineHeight}px`,
											margin: 0,
											minHeight: "100%",
											padding: `${paddingTop}px 16px ${paddingBottom}px 16px`,
											whiteSpace: wrap ? "pre-wrap" : "pre",
											wordBreak: wrap ? "break-word" : "normal",
										}}
									>
										{renderHighlightedLines({
											tokens,
											getLineProps,
											getTokenProps,
										})}
									</pre>
								)}
							</Highlight>
						</div>
					</div>
				</div>
			) : (
				<div className="relative h-full w-full overflow-hidden">
					{showLineNumbers ? (
						<div
							className="absolute top-0 bottom-0 left-0 overflow-hidden border-r px-2 text-right select-none"
							style={{
								background: palette.gutterBackground,
								borderColor: palette.border,
								color: palette.gutterForeground,
								fontFamily: MONO_FONT_FAMILY,
								fontSize,
								width: gutterWidth,
							}}
						>
							<div ref={gutterContentRef} style={{ paddingTop }}>
								{renderLineNumbers(lineCount, lineHeight)}
							</div>
						</div>
					) : null}
					<div
						className="absolute inset-0 overflow-hidden"
						style={{ left: gutterWidth }}
					>
						<div className="pointer-events-none absolute inset-0 overflow-hidden">
							<div ref={overlayContentRef}>
								<Highlight
									prism={Prism}
									theme={palette.theme}
									code={prismValue}
									language={resolvedPrismLanguage}
								>
									{({
										className,
										style,
										tokens,
										getLineProps,
										getTokenProps,
									}) => (
										<pre
											aria-hidden="true"
											className={className}
											style={{
												...style,
												background: "transparent",
												fontFamily: MONO_FONT_FAMILY,
												fontSize,
												lineHeight: `${lineHeight}px`,
												margin: 0,
												minHeight: "100%",
												padding: `${paddingTop}px 16px ${paddingBottom}px 16px`,
												whiteSpace: wrap ? "pre-wrap" : "pre",
												wordBreak: wrap ? "break-word" : "normal",
											}}
										>
											{renderHighlightedLines({
												tokens,
												getLineProps,
												getTokenProps,
											})}
										</pre>
									)}
								</Highlight>
							</div>
						</div>
						<textarea
							ref={textareaRef}
							aria-label="Code editor"
							autoCapitalize="off"
							autoCorrect="off"
							className="code-preview-editor-input absolute inset-0 h-full w-full resize-none border-0 bg-transparent outline-none"
							spellCheck={false}
							wrap={wrap ? "soft" : "off"}
							value={value}
							onChange={(event) => onChange?.(event.currentTarget.value)}
							onKeyDown={(event) => {
								if (event.key === "Tab" && !event.shiftKey) {
									event.preventDefault();
									onChange?.(insertTabAtSelection(event.currentTarget));
									return;
								}

								const keybinding = getKeybindingFromEvent(event);
								const command = commandsRef.current.get(keybinding);

								if (!command) {
									return;
								}

								event.preventDefault();
								command();
							}}
							onScroll={(event) => {
								const { scrollLeft, scrollTop } = event.currentTarget;

								if (overlayContentRef.current) {
									overlayContentRef.current.style.transform = `translate(${-scrollLeft}px, ${-scrollTop}px)`;
								}

								if (gutterContentRef.current) {
									gutterContentRef.current.style.transform = `translateY(${-scrollTop}px)`;
								}
							}}
							style={{
								WebkitTextFillColor: "transparent",
								caretColor: palette.caret,
								color: "transparent",
								fontFamily: MONO_FONT_FAMILY,
								fontSize,
								lineHeight: `${lineHeight}px`,
								padding: `${paddingTop}px 16px ${paddingBottom}px 16px`,
								tabSize: 4,
								whiteSpace: wrap ? "pre-wrap" : "pre",
								wordBreak: wrap ? "break-word" : "normal",
							}}
						/>
						<style>{`.code-preview-editor-input::selection { background: ${palette.selection}; }`}</style>
					</div>
				</div>
			)}
		</div>
	);
}
