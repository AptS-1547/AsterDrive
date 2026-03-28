import * as monaco from "monaco-editor";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";

type MonacoWorkerFactory = {
	getWorker: (_moduleId: string, label: string) => Worker;
};

let configured = false;

export function configureMonacoEnvironment() {
	if (configured) {
		return monaco;
	}

	const globalScope = self as typeof globalThis & {
		MonacoEnvironment?: MonacoWorkerFactory;
	};

	globalScope.MonacoEnvironment = {
		getWorker(_moduleId, label) {
			// Monaco only has dedicated language-service workers for a few families.
			// Most built-in languages are tokenizer-only and should fall back to editor.worker.
			switch (label) {
				case "json":
					return new jsonWorker();
				case "css":
				case "scss":
				case "less":
					return new cssWorker();
				case "html":
				case "handlebars":
				case "razor":
					return new htmlWorker();
				case "typescript":
				case "javascript":
					return new tsWorker();
				default:
					return new editorWorker();
			}
		},
	};

	configured = true;
	return monaco;
}

export { monaco };
