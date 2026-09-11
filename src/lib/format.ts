/** Format Indonesia: tanggal, waktu, rupiah. Penyimpanan tetap ISO. */

const dateFmt = new Intl.DateTimeFormat("id-ID", {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
});

const dateLongFmt = new Intl.DateTimeFormat("id-ID", {
  day: "numeric",
  month: "long",
  year: "numeric",
});

const dateTimeFmt = new Intl.DateTimeFormat("id-ID", {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});

const rupiahFmt = new Intl.NumberFormat("id-ID", {
  style: "currency",
  currency: "IDR",
  maximumFractionDigits: 0,
});

function toDate(v: string | Date): Date | null {
  const d = v instanceof Date ? v : new Date(v.length === 10 ? `${v}T00:00:00` : v);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** "11-09-2026". */
export function formatDate(v: string | Date): string {
  const d = toDate(v);
  return d ? dateFmt.format(d) : "-";
}

/** "11 September 2026". */
export function formatDateLong(v: string | Date): string {
  const d = toDate(v);
  return d ? dateLongFmt.format(d) : "-";
}

/** "11-09-2026 14.30". */
export function formatDateTime(v: string | Date): string {
  const d = toDate(v);
  return d ? dateTimeFmt.format(d) : "-";
}

/** "Rp 1.500.000". */
export function formatRupiah(v: number): string {
  return rupiahFmt.format(v);
}
