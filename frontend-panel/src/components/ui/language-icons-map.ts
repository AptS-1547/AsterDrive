import AngularPlainIcon from "@devicon/react/angular/plain";
import AstroPlainIcon from "@devicon/react/astro/plain";
import BashPlainIcon from "@devicon/react/bash/plain";
import CPlainIcon from "@devicon/react/c/plain";
import ClojureOriginalIcon from "@devicon/react/clojure/original";
import CoffeescriptOriginalIcon from "@devicon/react/coffeescript/original";
import CplusplusPlainIcon from "@devicon/react/cplusplus/plain";
import CsharpPlainIcon from "@devicon/react/csharp/plain";
import Css3PlainIcon from "@devicon/react/css3/plain";
import DartPlainIcon from "@devicon/react/dart/plain";
import DockerPlainIcon from "@devicon/react/docker/plain";
import ElixirPlainIcon from "@devicon/react/elixir/plain";
import ErlangPlainIcon from "@devicon/react/erlang/plain";
import GoPlainIcon from "@devicon/react/go/plain";
import GradleOriginalIcon from "@devicon/react/gradle/original";
import GraphqlPlainIcon from "@devicon/react/graphql/plain";
import GroovyPlainIcon from "@devicon/react/groovy/plain";
import GrpcPlainIcon from "@devicon/react/grpc/plain";
import HaskellPlainIcon from "@devicon/react/haskell/plain";
import Html5PlainIcon from "@devicon/react/html5/plain";
import JavaPlainIcon from "@devicon/react/java/plain";
import JavascriptPlainIcon from "@devicon/react/javascript/plain";
import JsonPlainIcon from "@devicon/react/json/plain";
import JuliaPlainIcon from "@devicon/react/julia/plain";
import KotlinPlainIcon from "@devicon/react/kotlin/plain";
import LatexOriginalIcon from "@devicon/react/latex/original";
import LuaPlainIcon from "@devicon/react/lua/plain";
import MarkdownOriginalIcon from "@devicon/react/markdown/original";
import NginxOriginalIcon from "@devicon/react/nginx/original";
import NimPlainIcon from "@devicon/react/nim/plain";
import NodejsPlainIcon from "@devicon/react/nodejs/plain";
import PerlPlainIcon from "@devicon/react/perl/plain";
import PhpPlainIcon from "@devicon/react/php/plain";
import PowershellPlainIcon from "@devicon/react/powershell/plain";
import PythonPlainIcon from "@devicon/react/python/plain";
import RPlainIcon from "@devicon/react/r/plain";
import ReactOriginalIcon from "@devicon/react/react/original";
import RubyPlainIcon from "@devicon/react/ruby/plain";
import RustOriginalIcon from "@devicon/react/rust/original";
import SassOriginalIcon from "@devicon/react/sass/original";
import ScalaPlainIcon from "@devicon/react/scala/plain";
import SolidityPlainIcon from "@devicon/react/solidity/plain";
import SqlitePlainIcon from "@devicon/react/sqlite/plain";
import SveltePlainIcon from "@devicon/react/svelte/plain";
import SwiftPlainIcon from "@devicon/react/swift/plain";
import TerraformPlainIcon from "@devicon/react/terraform/plain";
import TypescriptPlainIcon from "@devicon/react/typescript/plain";
import VuejsOriginalIcon from "@devicon/react/vuejs/original";
import XmlPlainIcon from "@devicon/react/xml/plain";
import YamlPlainIcon from "@devicon/react/yaml/plain";
import ZigOriginalIcon from "@devicon/react/zig/plain-wordmark";
import type { ComponentType } from "react";

export type DevIconComponent = ComponentType<{ size?: string | number }>;

const LANGUAGE_ICON_MAP: Record<string, DevIconComponent> = {
	// Web
	js: JavascriptPlainIcon,
	jsx: JavascriptPlainIcon,
	mjs: JavascriptPlainIcon,
	cjs: JavascriptPlainIcon,
	ts: TypescriptPlainIcon,
	tsx: TypescriptPlainIcon,
	vue: VuejsOriginalIcon,
	svelte: SveltePlainIcon,
	astro: AstroPlainIcon,
	html: Html5PlainIcon,
	htm: Html5PlainIcon,
	css: Css3PlainIcon,
	scss: SassOriginalIcon,
	less: SassOriginalIcon,
	svg: XmlPlainIcon,
	// Data / markup
	json: JsonPlainIcon,
	xml: XmlPlainIcon,
	yaml: YamlPlainIcon,
	yml: YamlPlainIcon,
	md: MarkdownOriginalIcon,
	markdown: MarkdownOriginalIcon,
	rst: MarkdownOriginalIcon,
	tex: LatexOriginalIcon,
	bib: LatexOriginalIcon,
	sql: SqlitePlainIcon,
	// Systems
	c: CPlainIcon,
	h: CPlainIcon,
	cpp: CplusplusPlainIcon,
	hpp: CplusplusPlainIcon,
	cs: CsharpPlainIcon,
	rs: RustOriginalIcon,
	go: GoPlainIcon,
	dart: DartPlainIcon,
	zig: ZigOriginalIcon,
	nim: NimPlainIcon,
	swift: SwiftPlainIcon,
	// JVM
	java: JavaPlainIcon,
	kt: KotlinPlainIcon,
	kts: KotlinPlainIcon,
	scala: ScalaPlainIcon,
	groovy: GroovyPlainIcon,
	clj: ClojureOriginalIcon,
	cljs: ClojureOriginalIcon,
	// Scripting
	py: PythonPlainIcon,
	rb: RubyPlainIcon,
	php: PhpPlainIcon,
	pl: PerlPlainIcon,
	pm: PerlPlainIcon,
	lua: LuaPlainIcon,
	r: RPlainIcon,
	jl: JuliaPlainIcon,
	coffee: CoffeescriptOriginalIcon,
	// Shell
	sh: BashPlainIcon,
	bash: BashPlainIcon,
	zsh: BashPlainIcon,
	fish: BashPlainIcon,
	ps1: PowershellPlainIcon,
	psm1: PowershellPlainIcon,
	bat: BashPlainIcon,
	cmd: BashPlainIcon,
	// Functional
	hs: HaskellPlainIcon,
	ex: ElixirPlainIcon,
	exs: ElixirPlainIcon,
	erl: ErlangPlainIcon,
	// Schema / query
	graphql: GraphqlPlainIcon,
	gql: GraphqlPlainIcon,
	proto: GrpcPlainIcon,
	// IaC / config
	tf: TerraformPlainIcon,
	tfvars: TerraformPlainIcon,
	hcl: TerraformPlainIcon,
	gradle: GradleOriginalIcon,
	// Web3
	sol: SolidityPlainIcon,
	// Infrastructure
	nginx: NginxOriginalIcon,
	// Frameworks
	react: ReactOriginalIcon,
	angular: AngularPlainIcon,
	nodejs: NodejsPlainIcon,
};

const SPECIAL_FILENAME_MAP: Record<string, DevIconComponent> = {
	dockerfile: DockerPlainIcon,
	".dockerignore": DockerPlainIcon,
	jenkinsfile: GroovyPlainIcon,
	vagrantfile: RubyPlainIcon,
	gemfile: RubyPlainIcon,
	rakefile: RubyPlainIcon,
	".npmrc": NginxOriginalIcon,
	".editorconfig": NginxOriginalIcon,
};

function getExtension(name: string) {
	const lower = name.trim().toLowerCase();
	const special = SPECIAL_FILENAME_MAP[lower];
	if (special) return { ext: lower, specialIcon: special };
	const dot = lower.lastIndexOf(".");
	if (dot < 0) return { ext: "", specialIcon: null };
	return { ext: lower.slice(dot + 1), specialIcon: null };
}

export function resolveIcon(name: string): DevIconComponent | null {
	const { ext, specialIcon } = getExtension(name);
	if (specialIcon) return specialIcon;
	return LANGUAGE_ICON_MAP[ext] ?? null;
}

export function checkHasIcon(name: string): boolean {
	return resolveIcon(name) !== null;
}
