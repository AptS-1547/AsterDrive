import { describe, expect, it } from "vitest";
import {
	buildCreateManagedIngressProfilePayload,
	buildUpdateManagedIngressProfilePayload,
	getManagedIngressProfileForm,
} from "@/components/admin/managedIngressProfileDialogShared";
import type { RemoteIngressProfileInfo } from "@/types/api";

describe("managedIngressProfileDialogShared", () => {
	it("maps an existing ingress profile into form state", () => {
		expect(
			getManagedIngressProfileForm({
				profile_key: "igp_demo",
				name: "Follower Cache",
				driver_type: "local",
				endpoint: "",
				bucket: "",
				base_path: "cache/inbox",
				max_file_size: 2048,
				is_default: true,
				desired_revision: 3,
				applied_revision: 3,
				last_error: "",
				created_at: "",
				updated_at: "",
			} as RemoteIngressProfileInfo),
		).toEqual({
			name: "Follower Cache",
			driver_type: "local",
			endpoint: "",
			bucket: "",
			access_key: "",
			secret_key: "",
			base_path: "cache/inbox",
			max_file_size: "2048",
			is_default: true,
		});
	});

	it("builds create payloads with normalized s3 fields", () => {
		expect(
			buildCreateManagedIngressProfilePayload({
				name: "Archive",
				driver_type: "s3",
				endpoint: "https://demo.r2.cloudflarestorage.com/uploads",
				bucket: "",
				access_key: "ACCESS",
				secret_key: "SECRET",
				base_path: "tenant-a/incoming",
				max_file_size: "8192",
				is_default: false,
			}),
		).toEqual({
			name: "Archive",
			driver_type: "s3",
			endpoint: "https://demo.r2.cloudflarestorage.com",
			bucket: "uploads",
			access_key: "ACCESS",
			secret_key: "SECRET",
			base_path: "tenant-a/incoming",
			max_file_size: 8192,
			is_default: false,
		});
	});

	it("omits unchanged s3 credentials from update payloads", () => {
		expect(
			buildUpdateManagedIngressProfilePayload(
				{
					name: "Archive",
					driver_type: "s3",
					endpoint: "https://demo.r2.cloudflarestorage.com/uploads",
					bucket: "",
					access_key: "",
					secret_key: "",
					base_path: "tenant-a/incoming",
					max_file_size: "",
					is_default: true,
				},
				{
					profile_key: "igp_archive",
					name: "Archive",
					driver_type: "s3",
					endpoint: "https://demo.r2.cloudflarestorage.com",
					bucket: "uploads",
					base_path: "tenant-a/incoming",
					max_file_size: 1024,
					is_default: false,
					desired_revision: 2,
					applied_revision: 2,
					last_error: "",
					created_at: "",
					updated_at: "",
				} as RemoteIngressProfileInfo,
			),
		).toEqual({
			name: "Archive",
			driver_type: "s3",
			endpoint: "https://demo.r2.cloudflarestorage.com",
			bucket: "uploads",
			base_path: "tenant-a/incoming",
			max_file_size: 0,
			is_default: true,
		});
	});

	it("requires explicit credentials when switching from local to s3", () => {
		expect(
			buildUpdateManagedIngressProfilePayload(
				{
					name: "Promoted",
					driver_type: "s3",
					endpoint: "https://s3.example.com",
					bucket: "bucket-a",
					access_key: "ROTATED",
					secret_key: "SECRET",
					base_path: "tenant-a/incoming",
					max_file_size: "4096",
					is_default: false,
				},
				{
					profile_key: "igp_local",
					name: "Promoted",
					driver_type: "local",
					endpoint: "",
					bucket: "",
					base_path: ".",
					max_file_size: 0,
					is_default: true,
					desired_revision: 1,
					applied_revision: 1,
					last_error: "",
					created_at: "",
					updated_at: "",
				} as RemoteIngressProfileInfo,
			),
		).toEqual({
			name: "Promoted",
			driver_type: "s3",
			endpoint: "https://s3.example.com",
			bucket: "bucket-a",
			access_key: "ROTATED",
			secret_key: "SECRET",
			base_path: "tenant-a/incoming",
			max_file_size: 4096,
			is_default: false,
		});
	});
});
