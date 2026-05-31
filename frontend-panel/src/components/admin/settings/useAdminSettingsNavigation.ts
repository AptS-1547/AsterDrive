import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { IconName } from "@/components/ui/icon";

type CategorySummary = {
	category: string;
	description?: string;
	icon: IconName;
	label: string;
};

function areCategoryListsEqual(left: string[], right: string[]) {
	return (
		left.length === right.length &&
		left.every((category, index) => category === right[index])
	);
}

function getCategoryRouteSegment(category: string) {
	return category.replaceAll("_", "-");
}

interface UseAdminSettingsNavigationProps {
	categoryIndex: Record<string, number>;
	categorySummaries: CategorySummary[];
	compactNavOverflowGap: number;
	compactNavTabGap: number;
	desktopBreakpoint: number;
	hasAnyConfig: boolean;
	loading: boolean;
	mobileBreakpoint: number;
	navigate: (
		to: string,
		options?: {
			replace?: boolean;
			viewTransition?: boolean;
		},
	) => void;
	resolvedSection: string;
	section: string;
	tabCategories: string[];
}

export function useAdminSettingsNavigation({
	categoryIndex,
	categorySummaries,
	compactNavOverflowGap,
	compactNavTabGap,
	desktopBreakpoint,
	hasAnyConfig,
	loading,
	mobileBreakpoint,
	navigate,
	resolvedSection,
	section,
	tabCategories,
}: UseAdminSettingsNavigationProps) {
	const compactNavContainerRef = useRef<HTMLDivElement | null>(null);
	const compactTabMeasureRefs = useRef<
		Record<string, HTMLButtonElement | null>
	>({});
	const compactOverflowMeasureRefs = useRef<
		Record<string, HTMLButtonElement | null>
	>({});
	const compactOverflowDefaultMeasureRef = useRef<HTMLButtonElement | null>(
		null,
	);
	const [viewportWidth, setViewportWidth] = useState(() =>
		typeof window === "undefined" ? desktopBreakpoint : window.innerWidth,
	);
	const [activeTab, setActiveTab] = useState<string>(section);
	const [tabDirection, setTabDirection] = useState<"forward" | "backward">(
		"forward",
	);
	const [compactInlineCategories, setCompactInlineCategories] = useState<
		string[]
	>([]);

	const categorySummaryMap = useMemo(
		() =>
			new Map(
				categorySummaries.map(
					(summary) => [summary.category, summary] as const,
				),
			),
		[categorySummaries],
	);

	const activeCategorySummary =
		categorySummaryMap.get(activeTab) ?? categorySummaries[0] ?? null;

	const compactOrderedSummaries = useMemo<CategorySummary[]>(
		() =>
			tabCategories.flatMap((category) => {
				const summary = categorySummaryMap.get(category);
				return summary ? [summary] : [];
			}),
		[categorySummaryMap, tabCategories],
	);

	const compactOverflowSummaries = useMemo<CategorySummary[]>(
		() =>
			compactOrderedSummaries.filter(
				(summary) => !compactInlineCategories.includes(summary.category),
			),
		[compactInlineCategories, compactOrderedSummaries],
	);

	const compactInlineSummaries = useMemo<CategorySummary[]>(
		() =>
			compactOrderedSummaries.filter((summary) =>
				compactInlineCategories.includes(summary.category),
			),
		[compactInlineCategories, compactOrderedSummaries],
	);

	const compactOverflowActiveSummary = useMemo(
		() =>
			compactOverflowSummaries.find(
				(summary) => summary.category === activeTab,
			) ?? null,
		[activeTab, compactOverflowSummaries],
	);

	const isMobileNavigation = viewportWidth < mobileBreakpoint;
	const isDesktopNavigation = viewportWidth >= desktopBreakpoint;
	const isCompactNavigation = !isMobileNavigation && !isDesktopNavigation;

	useEffect(() => {
		if (typeof window === "undefined") return;

		const updateViewportWidth = () => {
			setViewportWidth(window.innerWidth);
		};

		updateViewportWidth();
		window.addEventListener("resize", updateViewportWidth);
		return () => window.removeEventListener("resize", updateViewportWidth);
	}, []);

	useEffect(() => {
		if (loading || !hasAnyConfig || tabCategories.length === 0) {
			return;
		}

		if (section !== resolvedSection) {
			navigate(`/admin/settings/${getCategoryRouteSegment(resolvedSection)}`, {
				replace: true,
			});
		}
	}, [
		hasAnyConfig,
		loading,
		navigate,
		resolvedSection,
		section,
		tabCategories,
	]);

	useEffect(() => {
		if (activeTab === resolvedSection) {
			return;
		}

		setTabDirection(
			(categoryIndex[resolvedSection] ?? Number.MAX_SAFE_INTEGER) >=
				(categoryIndex[activeTab] ?? Number.MAX_SAFE_INTEGER)
				? "forward"
				: "backward",
		);
		setActiveTab(resolvedSection);
	}, [activeTab, categoryIndex, resolvedSection]);

	useEffect(() => {
		if (!isCompactNavigation) {
			setCompactInlineCategories((previous) =>
				areCategoryListsEqual(previous, tabCategories)
					? previous
					: tabCategories,
			);
			return;
		}

		const measureCompactNavigation = () => {
			const containerWidth = compactNavContainerRef.current?.clientWidth ?? 0;
			if (tabCategories.length === 0) {
				setCompactInlineCategories([]);
				return;
			}

			const tabWidths = tabCategories.map(
				(category) => compactTabMeasureRefs.current[category]?.offsetWidth ?? 0,
			);

			if (containerWidth <= 0 || tabWidths.some((width) => width <= 0)) {
				setCompactInlineCategories((previous) =>
					areCategoryListsEqual(previous, tabCategories)
						? previous
						: tabCategories,
				);
				return;
			}

			const totalTabWidth =
				tabWidths.reduce((sum, width) => sum + width, 0) +
				Math.max(0, tabWidths.length - 1) * compactNavTabGap;

			if (totalTabWidth <= containerWidth) {
				setCompactInlineCategories((previous) =>
					areCategoryListsEqual(previous, tabCategories)
						? previous
						: tabCategories,
				);
				return;
			}

			const overflowTriggerWidth = Math.max(
				compactOverflowDefaultMeasureRef.current?.offsetWidth ?? 0,
				...tabCategories.map(
					(category) =>
						compactOverflowMeasureRefs.current[category]?.offsetWidth ?? 0,
				),
			);
			const availableWidth = Math.max(
				0,
				containerWidth - overflowTriggerWidth - compactNavOverflowGap,
			);

			let usedWidth = 0;
			const nextInlineCategories: string[] = [];

			for (let index = 0; index < tabCategories.length; index += 1) {
				const category = tabCategories[index];
				const width = tabWidths[index];
				const gap = nextInlineCategories.length > 0 ? compactNavTabGap : 0;

				if (
					nextInlineCategories.length === 0 ||
					usedWidth + gap + width <= availableWidth
				) {
					nextInlineCategories.push(category);
					usedWidth += gap + width;
				}
			}

			setCompactInlineCategories((previous) =>
				areCategoryListsEqual(previous, nextInlineCategories)
					? previous
					: nextInlineCategories,
			);
		};

		measureCompactNavigation();

		if (
			typeof ResizeObserver === "undefined" ||
			!compactNavContainerRef.current
		) {
			return;
		}

		const resizeObserver = new ResizeObserver(() => {
			measureCompactNavigation();
		});
		resizeObserver.observe(compactNavContainerRef.current);

		return () => {
			resizeObserver.disconnect();
		};
	}, [
		compactNavOverflowGap,
		compactNavTabGap,
		isCompactNavigation,
		tabCategories,
	]);

	const handleCategoryChange = useCallback(
		(value: string) => {
			if (value === activeTab || !tabCategories.includes(value)) {
				return;
			}

			setTabDirection(
				(categoryIndex[value] ?? Number.MAX_SAFE_INTEGER) >=
					(categoryIndex[activeTab] ?? Number.MAX_SAFE_INTEGER)
					? "forward"
					: "backward",
			);
			setActiveTab(value);
			navigate(`/admin/settings/${getCategoryRouteSegment(value)}`, {
				viewTransition: false,
			});
		},
		[activeTab, categoryIndex, navigate, tabCategories],
	);

	return {
		activeCategorySummary,
		activeTab,
		compactInlineSummaries,
		compactNavContainerRef,
		compactOrderedSummaries,
		compactOverflowActiveSummary,
		compactOverflowDefaultMeasureRef,
		compactOverflowMeasureRefs,
		compactOverflowSummaries,
		compactTabMeasureRefs,
		handleCategoryChange,
		isCompactNavigation,
		isDesktopNavigation,
		isMobileNavigation,
		tabDirection,
		viewportWidth,
	};
}
