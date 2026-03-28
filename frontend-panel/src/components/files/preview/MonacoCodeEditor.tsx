import { useEffect, useRef } from "react";
import { configureMonacoEnvironment, monaco } from "./monaco-environment";

configureMonacoEnvironment();

export type MonacoCodeEditorMountHandler = (
	editor: monaco.editor.IStandaloneCodeEditor,
	monacoApi: typeof monaco,
) => void;

interface MonacoCodeEditorProps {
	language: string;
	theme: string;
	value: string;
	onChange?: (value: string) => void;
	onMount?: MonacoCodeEditorMountHandler;
	options?: monaco.editor.IStandaloneEditorConstructionOptions;
}

export function MonacoCodeEditor({
	language,
	theme,
	value,
	onChange,
	onMount,
	options,
}: MonacoCodeEditorProps) {
	const containerRef = useRef<HTMLDivElement | null>(null);
	const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
	const modelRef = useRef<monaco.editor.ITextModel | null>(null);
	const onChangeRef = useRef(onChange);
	const onMountRef = useRef(onMount);

	useEffect(() => {
		onChangeRef.current = onChange;
	}, [onChange]);

	useEffect(() => {
		onMountRef.current = onMount;
	}, [onMount]);

	useEffect(() => {
		const container = containerRef.current;
		if (!container) {
			return;
		}

		monaco.editor.setTheme(theme);

		const model = monaco.editor.createModel(value, language);
		const editor = monaco.editor.create(container, {
			...options,
			automaticLayout: true,
			model,
		});
		const subscription = editor.onDidChangeModelContent(() => {
			onChangeRef.current?.(model.getValue());
		});

		editorRef.current = editor;
		modelRef.current = model;
		onMountRef.current?.(editor, monaco);

		return () => {
			subscription.dispose();
			editor.dispose();
			model.dispose();
			editorRef.current = null;
			modelRef.current = null;
		};
	}, []);

	useEffect(() => {
		monaco.editor.setTheme(theme);
	}, [theme]);

	useEffect(() => {
		const editor = editorRef.current;
		if (!editor) {
			return;
		}

		editor.updateOptions({
			...options,
			automaticLayout: true,
		});
	}, [options]);

	useEffect(() => {
		const model = modelRef.current;
		if (!model) {
			return;
		}

		if (model.getLanguageId() !== language) {
			monaco.editor.setModelLanguage(model, language);
		}
	}, [language]);

	useEffect(() => {
		const editor = editorRef.current;
		const model = modelRef.current;
		if (!editor || !model) {
			return;
		}

		if (model.getValue() === value) {
			return;
		}

		editor.pushUndoStop();
		editor.executeEdits("props", [
			{
				range: model.getFullModelRange(),
				text: value,
				forceMoveMarkers: true,
			},
		]);
		editor.pushUndoStop();
	}, [value]);

	return <div ref={containerRef} className="h-full w-full" />;
}
