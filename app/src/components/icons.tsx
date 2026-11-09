import type { ReactNode, SVGProps } from 'react'

type IconProps = SVGProps<SVGSVGElement> & { size?: number }

function Icon({ size = 14, children, ...rest }: IconProps & { children: ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...rest}
    >
      {children}
    </svg>
  )
}

export const ChevronRight = (p: IconProps) => (
  <Icon {...p}>
    <path d="m9 6 6 6-6 6" />
  </Icon>
)

export const FolderIcon = (p: IconProps) => (
  <Icon {...p}>
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
  </Icon>
)

export const FileIcon = (p: IconProps) => (
  <Icon {...p}>
    <path d="M6 3h7l5 5v13H6Z" />
    <path d="M13 3v5h5" />
  </Icon>
)

export const SearchIcon = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="10.5" cy="10.5" r="6.5" />
    <path d="m16 16 4 4" />
  </Icon>
)

export const HomeIcon = (p: IconProps) => (
  <Icon {...p}>
    <path d="m3 11 9-8 9 8" />
    <path d="M5 10v10h14V10" />
  </Icon>
)

export const ArrowUpIcon = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 19V5" />
    <path d="m6 11 6-6 6 6" />
  </Icon>
)

export const TrashIcon = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 7h16" />
    <path d="M9 7V4h6v3" />
    <path d="M6 7l1 14h10l1-14" />
  </Icon>
)

export const XIcon = (p: IconProps) => (
  <Icon {...p}>
    <path d="M6 6l12 12M18 6 6 18" />
  </Icon>
)

export const ExternalIcon = (p: IconProps) => (
  <Icon {...p}>
    <path d="M14 4h6v6" />
    <path d="M20 4 11 13" />
    <path d="M19 14v6H4V5h6" />
  </Icon>
)

export const CopyIcon = (p: IconProps) => (
  <Icon {...p}>
    <rect x="9" y="9" width="11" height="11" rx="2" />
    <path d="M5 15V6a2 2 0 0 1 2-2h9" />
  </Icon>
)

export const CheckIcon = (p: IconProps) => (
  <Icon {...p}>
    <path d="m5 12 5 5L20 7" />
  </Icon>
)
