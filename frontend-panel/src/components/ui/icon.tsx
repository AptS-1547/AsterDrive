import type { ComponentType } from "react";
import {
	PiArrowClockwise,
	PiArrowCounterClockwise,
	PiArrowDown,
	PiArrowLeft,
	PiArrowSquareOut,
	PiArrowsClockwise,
	PiArrowsOutCardinal,
	PiArrowUp,
	PiArrowUUpLeft,
	PiBracketsCurly,
	PiCaretDown,
	PiCaretLeft,
	PiCaretRight,
	PiCaretUp,
	PiCheck,
	PiClipboardText,
	PiClockCounterClockwise,
	PiCloud,
	PiCopy,
	PiDotsThree,
	PiDownloadSimple,
	PiEye,
	PiEyeSlash,
	PiFile,
	PiFileAudio,
	PiFileCode,
	PiFilePlus,
	PiFileText,
	PiFileVideo,
	PiFileZip,
	PiFloppyDisk,
	PiFolder,
	PiFolderOpen,
	PiFolderPlus,
	PiGear,
	PiGlobe,
	PiGridFour,
	PiHardDrive,
	PiHouse,
	PiInfo,
	PiLink,
	PiLinkSimple,
	PiList,
	PiListBullets,
	PiLock,
	PiLockOpen,
	PiMagnifyingGlass,
	PiMinus,
	PiMonitor,
	PiMoon,
	PiPencilSimple,
	PiPlus,
	PiPower,
	PiPresentation,
	PiScroll,
	PiShield,
	PiSignIn,
	PiSignOut,
	PiSortAscending,
	PiSortDescending,
	PiSpinner,
	PiSun,
	PiTable,
	PiTrash,
	PiUploadSimple,
	PiWarning,
	PiWarningCircle,
	PiWifiHigh,
	PiWifiX,
	PiX,
} from "react-icons/pi";

export type IconName =
	| "ArrowCounterClockwise"
	| "ArrowClockwise"
	| "ArrowDown"
	| "ArrowLeft"
	| "ArrowSquareOut"
	| "ArrowUp"
	| "ArrowsClockwise"
	| "ArrowsOutCardinal"
	| "BracketsCurly"
	| "CaretDown"
	| "CaretLeft"
	| "CaretRight"
	| "CaretUp"
	| "Check"
	| "CircleAlert"
	| "ClipboardText"
	| "Clock"
	| "Cloud"
	| "Copy"
	| "DotsThree"
	| "Download"
	| "Eye"
	| "EyeSlash"
	| "File"
	| "FileAudio"
	| "FileCode"
	| "FilePlus"
	| "FileText"
	| "FileVideo"
	| "FileZip"
	| "FloppyDisk"
	| "Folder"
	| "FolderOpen"
	| "FolderPlus"
	| "Gear"
	| "Globe"
	| "Grid"
	| "HardDrive"
	| "House"
	| "Info"
	| "Link"
	| "LinkSimple"
	| "List"
	| "ListBullets"
	| "Lock"
	| "LockOpen"
	| "MagnifyingGlass"
	| "Monitor"
	| "Moon"
	| "Minus"
	| "PencilSimple"
	| "Plus"
	| "Power"
	| "Presentation"
	| "Scroll"
	| "Shield"
	| "SignIn"
	| "SignOut"
	| "SortAscending"
	| "SortDescending"
	| "Spinner"
	| "Sun"
	| "Table"
	| "Trash"
	| "Undo"
	| "Upload"
	| "Warning"
	| "WifiHigh"
	| "WifiX"
	| "X";

const iconMap: Record<IconName, ComponentType<{ className?: string }>> = {
	ArrowCounterClockwise: PiArrowCounterClockwise,
	ArrowClockwise: PiArrowClockwise,
	ArrowDown: PiArrowDown,
	ArrowLeft: PiArrowLeft,
	ArrowSquareOut: PiArrowSquareOut,
	ArrowUp: PiArrowUp,
	ArrowsClockwise: PiArrowsClockwise,
	ArrowsOutCardinal: PiArrowsOutCardinal,
	BracketsCurly: PiBracketsCurly,
	CaretDown: PiCaretDown,
	CaretLeft: PiCaretLeft,
	CaretRight: PiCaretRight,
	CaretUp: PiCaretUp,
	Check: PiCheck,
	CircleAlert: PiWarningCircle,
	ClipboardText: PiClipboardText,
	Clock: PiClockCounterClockwise,
	Cloud: PiCloud,
	Copy: PiCopy,
	DotsThree: PiDotsThree,
	Download: PiDownloadSimple,
	Eye: PiEye,
	EyeSlash: PiEyeSlash,
	File: PiFile,
	FileAudio: PiFileAudio,
	FileCode: PiFileCode,
	FilePlus: PiFilePlus,
	FileText: PiFileText,
	FileVideo: PiFileVideo,
	FileZip: PiFileZip,
	FloppyDisk: PiFloppyDisk,
	Folder: PiFolder,
	FolderOpen: PiFolderOpen,
	FolderPlus: PiFolderPlus,
	Gear: PiGear,
	Globe: PiGlobe,
	Grid: PiGridFour,
	HardDrive: PiHardDrive,
	House: PiHouse,
	Info: PiInfo,
	Link: PiLink,
	LinkSimple: PiLinkSimple,
	List: PiList,
	ListBullets: PiListBullets,
	Lock: PiLock,
	LockOpen: PiLockOpen,
	MagnifyingGlass: PiMagnifyingGlass,
	Monitor: PiMonitor,
	Moon: PiMoon,
	Minus: PiMinus,
	PencilSimple: PiPencilSimple,
	Plus: PiPlus,
	Power: PiPower,
	Presentation: PiPresentation,
	Scroll: PiScroll,
	Shield: PiShield,
	SignIn: PiSignIn,
	SignOut: PiSignOut,
	SortAscending: PiSortAscending,
	SortDescending: PiSortDescending,
	Spinner: PiSpinner,
	Sun: PiSun,
	Table: PiTable,
	Trash: PiTrash,
	Undo: PiArrowUUpLeft,
	Upload: PiUploadSimple,
	Warning: PiWarning,
	WifiHigh: PiWifiHigh,
	WifiX: PiWifiX,
	X: PiX,
};

export interface IconProps {
	name: IconName;
	className?: string;
}

export function Icon({ name, className }: IconProps) {
	const IconComponent = iconMap[name];
	if (!IconComponent) return null;
	return <IconComponent className={className} />;
}
