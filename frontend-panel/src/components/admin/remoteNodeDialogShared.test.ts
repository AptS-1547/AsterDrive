import { describe, expect, it } from "vitest";
import {
	buildCreateRemoteNodePayload,
	buildUpdateRemoteNodePayload,
	getRemoteNodeBaseUrlValidationMessage,
	getRemoteNodeForm,
} from "@/components/admin/remoteNodeDialogShared";
import type { RemoteNodeInfo } from "@/types/api";

describe("remoteNodeDialogShared", () => {
	it("maps an existing remote node into form state", () => {
		expect(
			getRemoteNodeForm({
				id: 4,
				name: "Edge Alpha",
				base_url: "https://remote.example.com",
				namespace: "tenant-a",
				is_enabled: true,
				last_error: "",
				last_checked_at: null,
				capabilities: {
					protocol_version: "v1",
					supports_list: true,
					supports_range_read: true,
					supports_stream_upload: true,
				},
				created_at: "",
				updated_at: "",
			} as RemoteNodeInfo),
		).toEqual({
			name: "Edge Alpha",
			base_url: "https://remote.example.com",
			namespace: "tenant-a",
			is_enabled: true,
		});
	});

	it("builds create payloads", () => {
		expect(
			buildCreateRemoteNodePayload({
				name: "Edge Alpha",
				base_url: "https://remote.example.com",
				namespace: "tenant-a",
				is_enabled: true,
			}),
		).toEqual({
			name: "Edge Alpha",
			base_url: "https://remote.example.com",
			namespace: "tenant-a",
			is_enabled: true,
		});
	});

	it("builds update payloads without managed credentials", () => {
		expect(
			buildUpdateRemoteNodePayload({
				name: "Edge Alpha",
				base_url: "",
				namespace: "tenant-b",
				is_enabled: false,
			}),
		).toEqual({
			name: "Edge Alpha",
			base_url: "",
			namespace: "tenant-b",
			is_enabled: false,
		});
	});

	it("allows an empty remote node base URL", () => {
		expect(
			getRemoteNodeBaseUrlValidationMessage("   ", (key) => key),
		).toBeNull();
	});

	it("rejects remote node base URLs that are not absolute http or https URLs", () => {
		expect(
			getRemoteNodeBaseUrlValidationMessage("remote.example.com", (key) => key),
		).toBe("remote_node_base_url_invalid");
		expect(
			getRemoteNodeBaseUrlValidationMessage(
				"ftp://remote.example.com",
				(key) => key,
			),
		).toBe("remote_node_base_url_invalid");
	});

	it("accepts absolute http and https remote node base URLs", () => {
		expect(
			getRemoteNodeBaseUrlValidationMessage(
				"https://remote.example.com/api",
				(key) => key,
			),
		).toBeNull();
		expect(
			getRemoteNodeBaseUrlValidationMessage(
				"http://remote.example.com",
				(key) => key,
			),
		).toBeNull();
	});
});
