import { UserAvatarImage } from "@/components/common/UserAvatarImage";
import { getUserDisplayName } from "@/lib/user";
import { cn } from "@/lib/utils";
import type { UserSummary } from "@/types/api";

interface UserIdentityProps {
	avatarClassName?: string;
	className?: string;
	fallbackLabel?: string;
	showUsername?: boolean;
	size?: "sm" | "md";
	user?: UserSummary | null;
}

const AVATAR_SIZE_CLASS = {
	sm: "size-7 rounded-xl text-[10px]",
	md: "size-8 rounded-xl text-xs",
} as const;

export function UserIdentity({
	avatarClassName,
	className,
	fallbackLabel = "-",
	showUsername = true,
	size = "sm",
	user,
}: UserIdentityProps) {
	if (!user) {
		return (
			<span className="text-sm text-muted-foreground">{fallbackLabel}</span>
		);
	}

	const displayName = getUserDisplayName(user);
	const shouldShowUsername = showUsername && displayName !== user.username;

	return (
		<div className={cn("flex min-w-0 items-center gap-2", className)}>
			<UserAvatarImage
				avatar={user.profile.avatar}
				name={displayName}
				size="sm"
				className={cn(AVATAR_SIZE_CLASS[size], avatarClassName)}
			/>
			<div className="min-w-0">
				<p className="truncate text-sm font-medium text-foreground">
					{displayName}
				</p>
				{shouldShowUsername ? (
					<p className="truncate text-xs text-muted-foreground">
						@{user.username}
					</p>
				) : null}
			</div>
		</div>
	);
}
