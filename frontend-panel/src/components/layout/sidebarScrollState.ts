export const userSidebarScroll = { value: 0 };

export function getUserSidebarScrollTop() {
	return userSidebarScroll.value;
}

export function setUserSidebarScrollTop(scrollTop: number) {
	userSidebarScroll.value = scrollTop;
}
