export function readLocalStorage(key: string): string | null {
	if (typeof window === "undefined") {
		return null;
	}

	try {
		return window.localStorage.getItem(key);
	} catch {
		return null;
	}
}

export function writeLocalStorage(key: string, value: string): void {
	if (typeof window === "undefined") {
		return;
	}

	try {
		window.localStorage.setItem(key, value);
	} catch {
		// localStorage can be blocked or full; preferences still update in memory.
	}
}
