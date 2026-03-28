import type { ComponentType } from "react";
import AngularOriginalIcon from "react-devicons/angular/original";
import AstroOriginalIcon from "react-devicons/astro/original";
import BashOriginalIcon from "react-devicons/bash/original";
import COriginalIcon from "react-devicons/c/original";
import ClojureOriginalIcon from "react-devicons/clojure/original";
import CoffeescriptOriginalIcon from "react-devicons/coffeescript/original";
import CplusplusOriginalIcon from "react-devicons/cplusplus/original";
import CsharpOriginalIcon from "react-devicons/csharp/original";
import Css3OriginalIcon from "react-devicons/css3/original";
import DartOriginalIcon from "react-devicons/dart/original";
import DockerOriginalIcon from "react-devicons/docker/original";
import ElixirOriginalIcon from "react-devicons/elixir/original";
import ErlangOriginalIcon from "react-devicons/erlang/original";
import GoOriginalIcon from "react-devicons/go/original";
import GradleOriginalIcon from "react-devicons/gradle/original";
import GraphqlPlainIcon from "react-devicons/graphql/plain";
import GroovyOriginalIcon from "react-devicons/groovy/original";
import GrpcOriginalIcon from "react-devicons/grpc/original";
import HaskellOriginalIcon from "react-devicons/haskell/original";
import Html5OriginalIcon from "react-devicons/html5/original";
import JavaOriginalIcon from "react-devicons/java/original";
import JavascriptOriginalIcon from "react-devicons/javascript/original";
import JsonOriginalIcon from "react-devicons/json/original";
import JuliaOriginalIcon from "react-devicons/julia/original";
import KotlinOriginalIcon from "react-devicons/kotlin/original";
import LatexOriginalIcon from "react-devicons/latex/original";
import LuaOriginalIcon from "react-devicons/lua/original";
import MarkdownOriginalIcon from "react-devicons/markdown/original";
import NginxOriginalIcon from "react-devicons/nginx/original";
import NimOriginalIcon from "react-devicons/nim/original";
import NodejsOriginalIcon from "react-devicons/nodejs/original";
import PerlOriginalIcon from "react-devicons/perl/original";
import PhpOriginalIcon from "react-devicons/php/original";
import PowershellOriginalIcon from "react-devicons/powershell/original";
import PythonOriginalIcon from "react-devicons/python/original";
import ROriginalIcon from "react-devicons/r/original";
import ReactOriginalIcon from "react-devicons/react/original";
import RubyOriginalIcon from "react-devicons/ruby/original";
import RustOriginalIcon from "react-devicons/rust/original";
import SassOriginalIcon from "react-devicons/sass/original";
import ScalaOriginalIcon from "react-devicons/scala/original";
import SolidityOriginalIcon from "react-devicons/solidity/original";
import SqliteOriginalIcon from "react-devicons/sqlite/original";
import SvelteOriginalIcon from "react-devicons/svelte/original";
import SwiftOriginalIcon from "react-devicons/swift/original";
import TerraformOriginalIcon from "react-devicons/terraform/original";
import TypescriptOriginalIcon from "react-devicons/typescript/original";
import VuejsOriginalIcon from "react-devicons/vuejs/original";
import XmlOriginalIcon from "react-devicons/xml/original";
import YamlOriginalIcon from "react-devicons/yaml/original";
import ZigOriginalIcon from "react-devicons/zig/original-wordmark";

export type DevIconComponent = ComponentType<{ size?: string | number }>;

const LANGUAGE_ICON_MAP: Record<string, DevIconComponent> = {
	// Web
	js: JavascriptOriginalIcon,
	jsx: JavascriptOriginalIcon,
	mjs: JavascriptOriginalIcon,
	cjs: JavascriptOriginalIcon,
	ts: TypescriptOriginalIcon,
	tsx: TypescriptOriginalIcon,
	vue: VuejsOriginalIcon,
	svelte: SvelteOriginalIcon,
	astro: AstroOriginalIcon,
	html: Html5OriginalIcon,
	htm: Html5OriginalIcon,
	css: Css3OriginalIcon,
	scss: SassOriginalIcon,
	less: SassOriginalIcon,
	svg: XmlOriginalIcon,
	// Data / markup
	json: JsonOriginalIcon,
	xml: XmlOriginalIcon,
	yaml: YamlOriginalIcon,
	yml: YamlOriginalIcon,
	md: MarkdownOriginalIcon,
	markdown: MarkdownOriginalIcon,
	rst: MarkdownOriginalIcon,
	tex: LatexOriginalIcon,
	bib: LatexOriginalIcon,
	sql: SqliteOriginalIcon,
	// Systems
	c: COriginalIcon,
	h: COriginalIcon,
	cpp: CplusplusOriginalIcon,
	hpp: CplusplusOriginalIcon,
	cs: CsharpOriginalIcon,
	rs: RustOriginalIcon,
	go: GoOriginalIcon,
	dart: DartOriginalIcon,
	zig: ZigOriginalIcon,
	nim: NimOriginalIcon,
	swift: SwiftOriginalIcon,
	// JVM
	java: JavaOriginalIcon,
	kt: KotlinOriginalIcon,
	kts: KotlinOriginalIcon,
	scala: ScalaOriginalIcon,
	groovy: GroovyOriginalIcon,
	clj: ClojureOriginalIcon,
	cljs: ClojureOriginalIcon,
	// Scripting
	py: PythonOriginalIcon,
	rb: RubyOriginalIcon,
	php: PhpOriginalIcon,
	pl: PerlOriginalIcon,
	pm: PerlOriginalIcon,
	lua: LuaOriginalIcon,
	r: ROriginalIcon,
	jl: JuliaOriginalIcon,
	coffee: CoffeescriptOriginalIcon,
	// Shell
	sh: BashOriginalIcon,
	bash: BashOriginalIcon,
	zsh: BashOriginalIcon,
	fish: BashOriginalIcon,
	ps1: PowershellOriginalIcon,
	psm1: PowershellOriginalIcon,
	bat: BashOriginalIcon,
	cmd: BashOriginalIcon,
	// Functional
	hs: HaskellOriginalIcon,
	ex: ElixirOriginalIcon,
	exs: ElixirOriginalIcon,
	erl: ErlangOriginalIcon,
	// Schema / query
	graphql: GraphqlPlainIcon,
	gql: GraphqlPlainIcon,
	proto: GrpcOriginalIcon,
	// IaC / config
	tf: TerraformOriginalIcon,
	tfvars: TerraformOriginalIcon,
	hcl: TerraformOriginalIcon,
	gradle: GradleOriginalIcon,
	// Web3
	sol: SolidityOriginalIcon,
	// Infrastructure
	nginx: NginxOriginalIcon,
	// Frameworks
	react: ReactOriginalIcon,
	angular: AngularOriginalIcon,
	nodejs: NodejsOriginalIcon,
};

const SPECIAL_FILENAME_MAP: Record<string, DevIconComponent> = {
	dockerfile: DockerOriginalIcon,
	".dockerignore": DockerOriginalIcon,
	jenkinsfile: GroovyOriginalIcon,
	vagrantfile: RubyOriginalIcon,
	gemfile: RubyOriginalIcon,
	rakefile: RubyOriginalIcon,
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
