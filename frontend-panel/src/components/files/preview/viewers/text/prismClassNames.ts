type PropsWithOptionalClassName = {
	className?: string;
};

export function scopePrismClassName(className?: string) {
	if (!className) {
		return className;
	}

	return className
		.trim()
		.split(/\s+/)
		.filter(Boolean)
		.map((name) => (name.startsWith("prism-") ? name : `prism-${name}`))
		.join(" ");
}

export function withScopedPrismClassName<T extends PropsWithOptionalClassName>(
	props: T,
) {
	const scopedClassName = scopePrismClassName(props.className);

	if (scopedClassName === props.className) {
		return props;
	}

	return {
		...props,
		className: scopedClassName,
	};
}
