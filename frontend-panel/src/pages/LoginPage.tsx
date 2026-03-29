import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import type { z } from "zod/v4";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { handleApiError } from "@/hooks/useApiError";
import { cn } from "@/lib/utils";
import { emailSchema, passwordSchema, usernameSchema } from "@/lib/validation";
import { authService } from "@/services/authService";
import { useAuthStore } from "@/stores/authStore";

// ── Animated height ─────────────────────────────────────────

function AnimateHeight({
	show,
	children,
}: {
	show: boolean;
	children: React.ReactNode;
}) {
	const [render, setRender] = useState(show);
	const [visible, setVisible] = useState(show);

	useEffect(() => {
		if (show) {
			setRender(true);
			requestAnimationFrame(() => {
				requestAnimationFrame(() => setVisible(true));
			});
		} else {
			setVisible(false);
		}
	}, [show]);

	const handleTransitionEnd = () => {
		if (!show) setRender(false);
	};

	if (!render) return null;

	return (
		<div
			className="grid transition-[grid-template-rows,opacity] duration-300 ease-out"
			style={{
				gridTemplateRows: visible ? "1fr" : "0fr",
				opacity: visible ? 1 : 0,
			}}
			onTransitionEnd={handleTransitionEnd}
		>
			<div className="overflow-hidden">{children}</div>
		</div>
	);
}

// ── Animated text ───────────────────────────────────────────

function AnimateText({
	text,
	className,
}: {
	text: string;
	className?: string;
}) {
	const [displayed, setDisplayed] = useState(text);
	const [animating, setAnimating] = useState(false);

	useEffect(() => {
		if (text === displayed) return;
		setAnimating(true);
		const timer = setTimeout(() => {
			setDisplayed(text);
			setAnimating(false);
		}, 150);
		return () => clearTimeout(timer);
	}, [text, displayed]);

	return (
		<span
			className={cn(
				"inline-block transition-all duration-150",
				animating ? "opacity-0 -translate-y-1" : "opacity-100 translate-y-0",
				className,
			)}
		>
			{displayed}
		</span>
	);
}

// ── Types ───────────────────────────────────────────────────

type AuthMode = "idle" | "login" | "register" | "setup";

// ── Component ───────────────────────────────────────────────

export default function LoginPage() {
	const { t } = useTranslation("auth");
	const navigate = useNavigate();
	const login = useAuthStore((s) => s.login);

	// The first field is always visible — it doubles as username or email
	const [identifier, setIdentifier] = useState("");
	// The extra field only shows for register/setup — it's whatever identifier is NOT
	const [extraField, setExtraField] = useState("");
	const [password, setPassword] = useState("");
	const [showPassword, setShowPassword] = useState(false);

	const [mode, setMode] = useState<AuthMode>("idle");
	const [checking, setChecking] = useState(false);
	const [submitting, setSubmitting] = useState(false);
	const [exiting, setExiting] = useState(false);
	const [errors, setErrors] = useState<Record<string, string>>({});

	const checkTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const lastChecked = useRef("");
	const latestCheckId = useRef(0);
	const latestIdentifier = useRef("");

	// Is the identifier an email address?
	const isEmail = identifier.includes("@");

	// In register/setup: identifier is one field, extraField is the other
	// If identifier is email → extraField is username (and vice versa)
	const identifierLabel = isEmail ? t("email") : t("username");
	const extraLabel = isEmail ? t("username") : t("email");
	const extraPlaceholder = isEmail ? t("choose_username") : "you@example.com";
	const requiresExtraField = mode === "register" || mode === "setup";
	const modeActionText =
		mode === "login"
			? t("sign_in")
			: mode === "register"
				? t("sign_up")
				: mode === "setup"
					? t("create_admin")
					: "";
	const isSubmitDisabled =
		submitting ||
		checking ||
		identifier.trim().length === 0 ||
		password.length === 0 ||
		(requiresExtraField && extraField.trim().length === 0);

	// ── Auto check ──

	const runCheck = useCallback(async (value: string) => {
		const trimmed = value.trim();
		if (trimmed.length < 2 || trimmed === lastChecked.current) return;

		const checkId = ++latestCheckId.current;
		setChecking(true);
		try {
			const result = await authService.check(trimmed);
			if (
				checkId !== latestCheckId.current ||
				latestIdentifier.current !== trimmed
			) {
				return;
			}
			lastChecked.current = trimmed;
			if (!result.has_users) {
				setMode("setup");
			} else if (result.exists) {
				setMode("login");
			} else {
				setMode("register");
			}
		} catch {
			// Silently fail
		} finally {
			if (checkId === latestCheckId.current) {
				setChecking(false);
			}
		}
	}, []);

	useEffect(() => {
		latestIdentifier.current = identifier.trim();
		if (checkTimer.current) clearTimeout(checkTimer.current);
		if (identifier.trim().length >= 2) {
			checkTimer.current = setTimeout(() => runCheck(identifier), 500);
		} else {
			setMode("idle");
			lastChecked.current = "";
		}
		return () => {
			if (checkTimer.current) clearTimeout(checkTimer.current);
		};
	}, [identifier, runCheck]);

	// ── Live validation ──

	const validateSingle = (field: string, value: string, schema: z.ZodType) => {
		const result = schema.safeParse(value);
		setErrors((prev) => {
			if (result.success) {
				const next = { ...prev };
				delete next[field];
				return next;
			}
			return { ...prev, [field]: result.error.issues[0]?.message ?? "" };
		});
	};

	// ── Submit validation ──

	const validate = (): boolean => {
		const errs: Record<string, string> = {};

		// Validate identifier as username or email
		const idSchema = isEmail ? emailSchema : usernameSchema;
		const idResult = idSchema.safeParse(identifier.trim());
		if (!idResult.success)
			errs.identifier = idResult.error.issues[0]?.message ?? "";

		// Validate extra field for register/setup
		if (mode === "register" || mode === "setup") {
			const extraSchema = isEmail ? usernameSchema : emailSchema;
			const extraResult = extraSchema.safeParse(extraField.trim());
			if (!extraResult.success)
				errs.extra = extraResult.error.issues[0]?.message ?? "";
		}

		const pwResult = passwordSchema.safeParse(password);
		if (!pwResult.success)
			errs.password = pwResult.error.issues[0]?.message ?? "";

		setErrors(errs);
		return Object.keys(errs).length === 0;
	};

	// ── Exit animation → navigate ──

	const exitAndNavigate = () => {
		setExiting(true);
		setTimeout(() => navigate("/", { replace: true }), 350);
	};

	// ── Submit ──

	const handleSubmit = async (e: React.FormEvent) => {
		e.preventDefault();
		if (!validate()) return;
		if (mode === "idle") {
			await runCheck(identifier);
			return;
		}

		setSubmitting(true);
		try {
			const id = identifier.trim();
			const extra = extraField.trim();

			if (mode === "login") {
				await login(id, password);
				exitAndNavigate();
			} else {
				const un = isEmail ? extra : id;
				const em = isEmail ? id : extra;

				if (mode === "setup") {
					await authService.setup(un, em, password);
					toast.success(t("setup_complete"));
				} else {
					await authService.register(un, em, password);
					toast.success(t("register_success"));
				}
				await login(em, password);
				exitAndNavigate();
			}
		} catch (error) {
			handleApiError(error);
			setSubmitting(false);
		}
	};

	// ── Labels ──

	const submitLabel = () => {
		if (submitting) {
			return mode === "login" ? t("signing_in") : t("creating_account");
		}
		if (mode === "setup") return t("create_admin");
		if (mode === "register") return t("sign_up");
		if (mode === "login") return t("sign_in");
		return t("continue");
	};

	const description = () => {
		if (mode === "setup") return t("setup_desc");
		if (mode === "register") return t("create_new_account");
		if (mode === "login") return t("enter_password");
		return t("sign_in_to_account");
	};

	return (
		<div
			className={cn(
				"min-h-screen flex transition-all duration-300 ease-out",
				exiting && "opacity-0 scale-[1.02]",
			)}
		>
			{/* Left — brand panel */}
			<div className="hidden lg:flex lg:w-1/2 bg-gradient-to-br from-neutral-900 via-neutral-800 to-neutral-900 items-center justify-center relative overflow-hidden">
				<div className="absolute inset-0 opacity-[0.03]">
					<div className="absolute top-1/4 left-1/4 w-96 h-96 rounded-full bg-emerald-500 blur-3xl" />
					<div className="absolute bottom-1/4 right-1/4 w-80 h-80 rounded-full bg-amber-500 blur-3xl" />
				</div>
				<div className="relative text-center px-12 max-w-md">
					<img
						src="/static/asterdrive/logo.svg"
						alt="AsterDrive"
						className="mx-auto h-20 w-auto"
					/>
					<p className="text-lg text-white/50 leading-relaxed">
						Your files, your server, your rules.
					</p>
				</div>
			</div>

			{/* Right — form */}
			<div className="flex-1 flex items-center justify-center bg-background p-6">
				<div className="w-full max-w-sm">
					{/* Mobile logo */}
					<div className="lg:hidden text-center mb-8">
						<img
							src="/static/asterdrive/logo.svg"
							alt="AsterDrive"
							className="mx-auto h-14 w-auto"
						/>
					</div>

					{/* Header */}
					<div className="mb-6 overflow-hidden">
						<h2 className="text-xl font-semibold tracking-tight">
							<AnimateText
								text={
									mode === "setup"
										? t("welcome_setup")
										: t("sign_in_to_account")
								}
							/>
						</h2>
						<p className="text-sm text-muted-foreground mt-1">
							<AnimateText text={description()} />
						</p>
					</div>

					<form onSubmit={handleSubmit}>
						{/* Field 1: identifier (email or username) — always visible */}
						<div className="space-y-1.5">
							<div className="flex items-center justify-between">
								<Label htmlFor="identifier" className="text-sm">
									<AnimateText
										text={
											mode === "register" || mode === "setup"
												? identifierLabel
												: t("email_or_username")
										}
									/>
								</Label>
								<div className="flex items-center gap-2">
									{mode !== "idle" && (
										<AnimateText
											text={modeActionText}
											className={cn(
												"text-xs text-muted-foreground/70",
												checking && "opacity-0",
											)}
										/>
									)}
									{checking && (
										<Icon
											name="Spinner"
											className="h-3 w-3 animate-spin text-muted-foreground"
										/>
									)}
								</div>
							</div>
							<Input
								id="identifier"
								placeholder="you@example.com"
								value={identifier}
								onChange={(e) => {
									const v = e.target.value;
									setIdentifier(v);
									// Live validate as username if not email
									if (v.length > 0 && !v.includes("@")) {
										validateSingle("identifier", v, usernameSchema);
									} else if (v.includes("@") && v.length > 3) {
										validateSingle("identifier", v, emailSchema);
									} else {
										setErrors((prev) => {
											const next = { ...prev };
											delete next.identifier;
											return next;
										});
									}
								}}
								required
								autoFocus
								autoComplete="username"
								className={cn(
									"h-10",
									errors.identifier &&
										"border-destructive focus-visible:ring-destructive",
								)}
							/>
							{errors.identifier && (
								<p className="text-xs text-destructive">{errors.identifier}</p>
							)}
						</div>

						{/* Field 2: extra field — only for register/setup */}
						<AnimateHeight show={mode === "register" || mode === "setup"}>
							<div className="space-y-1.5 mt-4">
								<Label htmlFor="extra" className="text-sm">
									<AnimateText text={extraLabel} />
								</Label>
								<Input
									id="extra"
									placeholder={extraPlaceholder}
									value={extraField}
									onChange={(e) => {
										const v = e.target.value;
										setExtraField(v);
										const schema = isEmail ? usernameSchema : emailSchema;
										validateSingle("extra", v, schema);
									}}
									required={mode === "register" || mode === "setup"}
									autoComplete={isEmail ? "off" : "email"}
									className={cn(
										"h-10",
										errors.extra &&
											"border-destructive focus-visible:ring-destructive",
									)}
								/>
								{errors.extra && (
									<p className="text-xs text-destructive">{errors.extra}</p>
								)}
							</div>
						</AnimateHeight>

						{/* Field 3: password — always visible */}
						<div className="space-y-1.5 mt-4">
							<Label htmlFor="password" className="text-sm">
								{t("password")}
							</Label>
							<div className="relative">
								<Input
									id="password"
									type={showPassword ? "text" : "password"}
									placeholder={t("password")}
									value={password}
									onChange={(e) => {
										setPassword(e.target.value);
										if (mode !== "login") {
											validateSingle(
												"password",
												e.target.value,
												passwordSchema,
											);
										}
									}}
									required
									autoComplete={
										mode === "login" ? "current-password" : "new-password"
									}
									className={cn(
										"h-10 pr-10",
										errors.password &&
											"border-destructive focus-visible:ring-destructive",
									)}
								/>
								<button
									type="button"
									className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
									onClick={() => setShowPassword(!showPassword)}
									tabIndex={-1}
									aria-label={
										showPassword
											? t("core:hide_password")
											: t("core:show_password")
									}
								>
									{showPassword ? (
										<Icon name="EyeSlash" className="h-4 w-4" />
									) : (
										<Icon name="Eye" className="h-4 w-4" />
									)}
								</button>
							</div>
							{errors.password && (
								<p className="text-xs text-destructive">{errors.password}</p>
							)}
						</div>

						<Button
							type="submit"
							className="w-full h-10 mt-4"
							disabled={isSubmitDisabled}
						>
							{submitting && (
								<Icon name="Spinner" className="h-4 w-4 animate-spin mr-2" />
							)}
							{submitLabel()}
						</Button>
					</form>

					<p className="mt-8 text-center text-xs text-muted-foreground/50">
						Self-hosted cloud storage
					</p>
				</div>
			</div>
		</div>
	);
}
