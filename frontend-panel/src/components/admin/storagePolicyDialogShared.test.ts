import { describe, expect, it } from "vitest";
import {
	buildCreatePolicyPayload,
	buildPolicyTestPayload,
	buildUpdatePolicyPayload,
	getPolicyForm,
} from "@/components/admin/storagePolicyDialogShared";
import type { StoragePolicy } from "@/types/api";

describe("storagePolicyDialogShared", () => {
	it("maps an existing policy into form state", () => {
		expect(
			getPolicyForm({
				id: 3,
				name: "Archive",
				driver_type: "local",
				endpoint: "",
				bucket: "",
				access_key: "",
				secret_key: "",
				base_path: "/data/archive",
				remote_node_id: null,
				max_file_size: 1024,
				allowed_types: [],
				options: {
					content_dedup: true,
					thumbnail_processor: "storage_native",
				},
				is_default: true,
				chunk_size: 10 * 1024 * 1024,
				created_at: "",
				updated_at: "",
			} as StoragePolicy),
		).toEqual({
			name: "Archive",
			driver_type: "local",
			endpoint: "",
			bucket: "",
			access_key: "",
			secret_key: "",
			base_path: "/data/archive",
			remote_node_id: "",
			max_file_size: "1024",
			chunk_size: "10",
			is_default: true,
			content_dedup: true,
			storage_native_thumbnail: true,
			remote_download_strategy: "relay_stream",
			remote_upload_strategy: "relay_stream",
			s3_upload_strategy: "relay_stream",
			s3_download_strategy: "relay_stream",
		});
	});

	it("builds create payloads with normalized S3 fields", () => {
		expect(
			buildCreatePolicyPayload({
				name: "Media",
				driver_type: "s3",
				endpoint: "https://demo.r2.cloudflarestorage.com/photos",
				bucket: "",
				access_key: "AKIA",
				secret_key: "SECRET",
				base_path: "videos",
				remote_node_id: "",
				max_file_size: "2048",
				chunk_size: "6",
				is_default: false,
				content_dedup: false,
				storage_native_thumbnail: true,
				remote_download_strategy: "relay_stream",
				remote_upload_strategy: "relay_stream",
				s3_upload_strategy: "presigned",
				s3_download_strategy: "relay_stream",
			}),
		).toEqual({
			name: "Media",
			driver_type: "s3",
			endpoint: "https://demo.r2.cloudflarestorage.com",
			bucket: "photos",
			access_key: "AKIA",
			secret_key: "SECRET",
			base_path: "videos",
			max_file_size: 2048,
			chunk_size: 6 * 1024 * 1024,
			is_default: false,
			options: {
				s3_upload_strategy: "presigned",
				s3_download_strategy: "relay_stream",
				thumbnail_processor: "storage_native",
			},
		});
	});

	it("omits empty credentials from update payloads", () => {
		expect(
			buildUpdatePolicyPayload({
				name: "Media",
				driver_type: "s3",
				endpoint: "https://example.com",
				bucket: "bucket-a",
				access_key: "",
				secret_key: "",
				base_path: "videos",
				remote_node_id: "",
				max_file_size: "",
				chunk_size: "5",
				is_default: true,
				content_dedup: false,
				storage_native_thumbnail: false,
				remote_download_strategy: "relay_stream",
				remote_upload_strategy: "relay_stream",
				s3_upload_strategy: "relay_stream",
				s3_download_strategy: "presigned",
			}),
		).toEqual({
			name: "Media",
			endpoint: "https://example.com",
			bucket: "bucket-a",
			base_path: "videos",
			max_file_size: undefined,
			chunk_size: 5 * 1024 * 1024,
			is_default: true,
			options: {
				s3_upload_strategy: "relay_stream",
				s3_download_strategy: "presigned",
			},
		});
	});

	it("builds remote payloads with remote node binding only", () => {
		expect(
			buildCreatePolicyPayload({
				name: "Remote Edge",
				driver_type: "remote",
				endpoint: "",
				bucket: "",
				access_key: "",
				secret_key: "",
				base_path: "tenant-a/uploads",
				remote_node_id: "9",
				max_file_size: "",
				chunk_size: "4",
				is_default: false,
				content_dedup: false,
				storage_native_thumbnail: false,
				remote_download_strategy: "presigned",
				remote_upload_strategy: "presigned",
				s3_upload_strategy: "relay_stream",
				s3_download_strategy: "relay_stream",
			}),
		).toEqual({
			name: "Remote Edge",
			driver_type: "remote",
			endpoint: "",
			bucket: "",
			access_key: "",
			secret_key: "",
			base_path: "tenant-a/uploads",
			remote_node_id: 9,
			max_file_size: undefined,
			chunk_size: 4 * 1024 * 1024,
			is_default: false,
			options: {
				remote_download_strategy: "presigned",
				remote_upload_strategy: "presigned",
			},
		});

		expect(
			buildPolicyTestPayload({
				name: "Remote Edge",
				driver_type: "remote",
				endpoint: "",
				bucket: "",
				access_key: "",
				secret_key: "",
				base_path: "tenant-a/uploads",
				remote_node_id: "9",
				max_file_size: "",
				chunk_size: "4",
				is_default: false,
				content_dedup: false,
				storage_native_thumbnail: false,
				remote_download_strategy: "presigned",
				remote_upload_strategy: "presigned",
				s3_upload_strategy: "relay_stream",
				s3_download_strategy: "relay_stream",
			}),
		).toEqual({
			driver_type: "remote",
			endpoint: undefined,
			bucket: undefined,
			access_key: undefined,
			secret_key: undefined,
			base_path: "tenant-a/uploads",
			remote_node_id: 9,
		});
	});
});
