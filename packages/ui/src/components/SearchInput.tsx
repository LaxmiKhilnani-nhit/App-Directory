import { forwardRef, type InputHTMLAttributes } from "react";

export interface SearchInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "onChange"> {
  value: string;
  onValueChange: (value: string) => void;
  placeholder?: string;
}

export const SearchInput = forwardRef<HTMLInputElement, SearchInputProps>(function SearchInput(
  { value, onValueChange, placeholder = "Search apps…", className, ...rest },
  ref,
) {
  return (
    <div className={["ad-search", className].filter(Boolean).join(" ")}>
      <span className="ad-search__icon" aria-hidden="true">
        <svg width="15" height="15" viewBox="0 0 15 15" fill="none">
          <circle cx="6.5" cy="6.5" r="4.75" stroke="currentColor" strokeWidth="1.5" />
          <path d="M10.5 10.5 13 13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      </span>
      <input
        ref={ref}
        type="search"
        className="ad-search__input"
        value={value}
        placeholder={placeholder}
        onChange={(event) => onValueChange(event.target.value)}
        {...rest}
      />
      {value ? (
        <button
          type="button"
          className="ad-search__clear"
          onClick={() => onValueChange("")}
          aria-label="Clear search"
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path
              d="M3.5 3.5l7 7m0-7l-7 7"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          </svg>
        </button>
      ) : null}
    </div>
  );
});
