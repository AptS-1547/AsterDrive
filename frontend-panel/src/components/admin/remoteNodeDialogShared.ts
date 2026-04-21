import type {
	CreateRemoteNodeRequest,
	PatchRemoteNodeReq,
	RemoteNodeInfo,
} from "@/types/api";

export interface RemoteNodeFormData {
	name: string;
	base_url: string;
	namespace: string;
	is_enabled: boolean;
}

export function getRemoteNodeForm(node: RemoteNodeInfo): RemoteNodeFormData {
	return {
		name: node.name,
		base_url: node.base_url,
		namespace: node.namespace,
		is_enabled: node.is_enabled,
	};
}

export function buildCreateRemoteNodePayload(
	form: RemoteNodeFormData,
): CreateRemoteNodeRequest {
	return {
		name: form.name,
		base_url: form.base_url || undefined,
		namespace: form.namespace,
		is_enabled: form.is_enabled,
	};
}

export function buildUpdateRemoteNodePayload(
	form: RemoteNodeFormData,
): PatchRemoteNodeReq {
	return {
		name: form.name,
		base_url: form.base_url,
		namespace: form.namespace,
		is_enabled: form.is_enabled,
	};
}

export function hasRemoteConnectionFieldChanges(
	form: RemoteNodeFormData,
	editingNode: RemoteNodeInfo | null,
) {
	if (!editingNode) {
		return true;
	}

	return form.base_url !== editingNode.base_url;
}

export function getRemoteNodeBaseUrlValidationMessage(
	baseUrl: string,
	t: (key: string) => string,
) {
	const trimmedBaseUrl = baseUrl.trim();
	if (!trimmedBaseUrl) {
		return null;
	}

	let parsedUrl: URL;
	try {
		parsedUrl = new URL(trimmedBaseUrl);
	} catch {
		return t("remote_node_base_url_invalid");
	}

	return parsedUrl.protocol === "http:" || parsedUrl.protocol === "https:"
		? null
		: t("remote_node_base_url_invalid");
}

export const emptyRemoteNodeForm: RemoteNodeFormData = {
	name: "",
	base_url: "",
	namespace: "",
	is_enabled: true,
};
