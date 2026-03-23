import { ArrowLeft, Eye, EyeOff, Loader2 } from "lucide-react";
import type { FormEvent } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { handleApiError } from "@/hooks/useApiError";
import { authService } from "@/services/authService";
import { useAuthStore } from "@/stores/authStore";

type Step = "identifier" | "login" | "register" | "setup";

export default function LoginPage() {
	const { t } = useTranslation("auth");
	const navigate = useNavigate();
	const login = useAuthStore((s) => s.login);

	const [step, setStep] = useState<Step>("identifier");
	const [identifier, setIdentifier] = useState("");
	const [username, setUsername] = useState("");
	const [email, setEmail] = useState("");
	const [password, setPassword] = useState("");
	const [showPassword, setShowPassword] = useState(false);
	const [loading, setLoading] = useState(false);

	// Step 1: Check identifier
	const handleCheck = async (e: FormEvent) => {
		e.preventDefault();
		if (!identifier.trim()) return;
		setLoading(true);
		try {
			const result = await authService.check(identifier.trim());
			if (!result.has_users) {
				// 首次设置 — 自动用输入的值填充
				if (identifier.includes("@")) {
					setEmail(identifier.trim());
				} else {
					setUsername(identifier.trim());
				}
				setStep("setup");
			} else if (result.exists) {
				setStep("login");
			} else {
				// 用户不存在但系统已有用户 → 注册
				if (identifier.includes("@")) {
					setEmail(identifier.trim());
				} else {
					setUsername(identifier.trim());
				}
				setStep("register");
			}
		} catch (error) {
			handleApiError(error);
		} finally {
			setLoading(false);
		}
	};

	// Step 2a: Login
	const handleLogin = async (e: FormEvent) => {
		e.preventDefault();
		setLoading(true);
		try {
			await login(identifier.trim(), password);
			navigate("/", { replace: true });
		} catch (error) {
			handleApiError(error);
		} finally {
			setLoading(false);
		}
	};

	// Step 2b: Register
	const handleRegister = async (e: FormEvent) => {
		e.preventDefault();
		setLoading(true);
		try {
			await authService.register(username, email, password);
			toast.success(t("register_success"));
			await login(identifier.trim(), password);
			navigate("/", { replace: true });
		} catch (error) {
			handleApiError(error);
		} finally {
			setLoading(false);
		}
	};

	// Step 2c: First-time setup
	const handleSetup = async (e: FormEvent) => {
		e.preventDefault();
		setLoading(true);
		try {
			await authService.setup(username, email, password);
			toast.success(t("setup_complete"));
			await login(email || username, password);
			navigate("/", { replace: true });
		} catch (error) {
			handleApiError(error);
		} finally {
			setLoading(false);
		}
	};

	const goBack = () => {
		setPassword("");
		setShowPassword(false);
		setStep("identifier");
	};

	const passwordField = (
		<div className="relative">
			<Input
				type={showPassword ? "text" : "password"}
				placeholder={t("password")}
				value={password}
				onChange={(e) => setPassword(e.target.value)}
				required
				autoFocus={step !== "identifier"}
				className="pr-10"
			/>
			<button
				type="button"
				className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
				onClick={() => setShowPassword(!showPassword)}
				tabIndex={-1}
				aria-label={
					showPassword ? t("common:hide_password") : t("common:show_password")
				}
			>
				{showPassword ? (
					<EyeOff className="h-4 w-4" />
				) : (
					<Eye className="h-4 w-4" />
				)}
			</button>
		</div>
	);

	return (
		<div className="min-h-screen flex items-center justify-center bg-background p-4">
			<Card className="w-full max-w-sm">
				<CardHeader>
					<CardTitle className="text-2xl text-center">
						{step === "setup" ? t("welcome_setup") : t("common:app_name")}
					</CardTitle>
					<CardDescription className="text-center">
						{step === "identifier" && t("sign_in_to_account")}
						{step === "login" && t("enter_password")}
						{step === "register" && t("create_new_account")}
						{step === "setup" && t("setup_desc")}
					</CardDescription>
				</CardHeader>
				<CardContent>
					{/* Step 1: Identifier */}
					{step === "identifier" && (
						<form onSubmit={handleCheck} className="space-y-4">
							<Input
								placeholder={t("email_or_username")}
								value={identifier}
								onChange={(e) => setIdentifier(e.target.value)}
								required
								autoFocus
							/>
							<Button type="submit" className="w-full" disabled={loading}>
								{loading ? (
									<Loader2 className="h-4 w-4 animate-spin" />
								) : (
									t("continue")
								)}
							</Button>
						</form>
					)}

					{/* Step 2a: Login */}
					{step === "login" && (
						<form onSubmit={handleLogin} className="space-y-4">
							<div className="flex items-center justify-between">
								<span className="text-sm font-medium">{identifier}</span>
								<button
									type="button"
									className="text-xs text-muted-foreground hover:text-foreground flex items-center gap-1"
									onClick={goBack}
								>
									<ArrowLeft className="h-3 w-3" />
									{t("not_you")}
								</button>
							</div>
							{passwordField}
							<Button type="submit" className="w-full" disabled={loading}>
								{loading ? t("signing_in") : t("sign_in")}
							</Button>
						</form>
					)}

					{/* Step 2b: Register */}
					{step === "register" && (
						<form onSubmit={handleRegister} className="space-y-4">
							<div className="flex items-center justify-between">
								<span className="text-sm text-muted-foreground">
									{identifier}
								</span>
								<button
									type="button"
									className="text-xs text-muted-foreground hover:text-foreground flex items-center gap-1"
									onClick={goBack}
								>
									<ArrowLeft className="h-3 w-3" />
									{t("common:back")}
								</button>
							</div>
							{!username && (
								<div className="space-y-1.5">
									<Label>{t("username")}</Label>
									<Input
										placeholder={t("choose_username")}
										value={username}
										onChange={(e) => setUsername(e.target.value)}
										required
										autoFocus
									/>
								</div>
							)}
							{!email && (
								<div className="space-y-1.5">
									<Label>{t("email")}</Label>
									<Input
										type="email"
										placeholder={t("email")}
										value={email}
										onChange={(e) => setEmail(e.target.value)}
										required
										autoFocus={!!username}
									/>
								</div>
							)}
							{passwordField}
							<Button type="submit" className="w-full" disabled={loading}>
								{loading ? t("creating_account") : t("sign_up")}
							</Button>
						</form>
					)}

					{/* Step 2c: First-time setup */}
					{step === "setup" && (
						<form onSubmit={handleSetup} className="space-y-4">
							{!username && (
								<div className="space-y-1.5">
									<Label>{t("username")}</Label>
									<Input
										placeholder={t("choose_username")}
										value={username}
										onChange={(e) => setUsername(e.target.value)}
										required
										autoFocus
									/>
								</div>
							)}
							{!email && (
								<div className="space-y-1.5">
									<Label>{t("email")}</Label>
									<Input
										type="email"
										placeholder={t("email")}
										value={email}
										onChange={(e) => setEmail(e.target.value)}
										required
										autoFocus={!!username}
									/>
								</div>
							)}
							{passwordField}
							<Button type="submit" className="w-full" disabled={loading}>
								{loading ? t("creating_account") : t("create_admin")}
							</Button>
						</form>
					)}
				</CardContent>
			</Card>
		</div>
	);
}
