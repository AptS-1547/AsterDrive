import { type ComponentType, type LazyExoticComponent, lazy } from "react";

// biome-ignore lint/suspicious/noExplicitAny: React.lazy is typed against ComponentType<any>; keeping that constraint preserves exact props/ref inference for lazy-loaded components.
type LazyCompatibleComponent = ComponentType<any>;

type LazyModule<T extends LazyCompatibleComponent> = {
	default: T;
};

export type PreloadableLazyComponent<T extends LazyCompatibleComponent> =
	LazyExoticComponent<T> & {
		preload: () => Promise<LazyModule<T>>;
	};

export function lazyWithPreload<T extends LazyCompatibleComponent>(
	load: () => Promise<LazyModule<T>>,
): PreloadableLazyComponent<T> {
	let cachedPromise: Promise<LazyModule<T>> | null = null;

	const preload = () => {
		cachedPromise ??= load();
		return cachedPromise;
	};

	const LazyComponent = lazy(preload) as PreloadableLazyComponent<T>;
	LazyComponent.preload = preload;
	return LazyComponent;
}
