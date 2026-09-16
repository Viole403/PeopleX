import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type FingerDeviceInput } from "../bindings";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/devices")({
  component: DevicesPage,
});

const BRANDS = [
  "ZKTeco",
  "Solution",
  "Fingerspot",
  "Revo",
  "Deli",
  "Krisbow",
  "eSSL",
  "Hikvision",
  "Dahua",
  "Anviz",
  "Suprema",
  "Matrix",
  "Nitgen",
  "Lainnya",
];

const PROTOCOLS = [
  { value: "adms", label: "ADMS Push (ZKTeco/Solution/dll)" },
  { value: "zk_pull", label: "ZK Pull port 4370 (LAN)" },
  { value: "usb", label: "USB / Berkas" },
  { value: "cloud", label: "Cloud Webhook" },
  { value: "agent", label: "Agen Kabel" },
];

const DRIVERS = ["universal", "cloud-webhook", "agent", "fingerspot-cloud", "deli-sdk", "hikvision-isapi"];

const EMPTY: FingerDeviceInput = {
  name: "",
  brand: "ZKTeco",
  model: null,
  serial: null,
  protocol: "adms",
  driver: null,
  endpoint: null,
  location_id: null,
  active: true,
};

function DevicesPage() {
  const { data: session } = useSession();
  const queryClient = useQueryClient();
  const [formOpen, setFormOpen] = useState(false);
  const [editId, setEditId] = useState<number | null>(null);
  const [form, setForm] = useState<FingerDeviceInput>(EMPTY);
  const editable = can(session, "attendance.approve");

  const devices = useQuery({
    queryKey: ["fingerprintDevices"],
    queryFn: () => unwrap(commands.fingerprintDevices()),
  });

  const save = useMutation({
    mutationFn: () => unwrap(commands.fingerprintDeviceSave(editId, form)),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["fingerprintDevices"] });
      setFormOpen(false);
      setEditId(null);
      setForm(EMPTY);
      toast.success("Perangkat tersimpan.");
    },
    onError: (e) => toast.error(String(e)),
  });

  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.fingerprintDeviceDelete(id)),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["fingerprintDevices"] });
      toast.success("Perangkat dihapus.");
    },
    onError: (e) => toast.error(String(e)),
  });

  const openAdd = () => {
    setEditId(null);
    setForm(EMPTY);
    setFormOpen(true);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-display-sm font-semibold">Perangkat Absensi</h1>
        {editable && (
          <button
            type="button"
            onClick={openAdd}
            className="rounded-lg bg-bg-brand-primary px-4 py-2 text-sm font-medium text-text-brand-primary"
          >
            Tambah Perangkat
          </button>
        )}
      </div>
      {devices.isError && <p className="text-sm text-text-error">Gagal memuat perangkat.</p>}
      <div className="overflow-x-auto rounded-xl border border-border-secondary">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-bg-secondary text-left">
              <th className="px-3 py-2">Nama</th>
              <th className="px-3 py-2">Merek</th>
              <th className="px-3 py-2">Model</th>
              <th className="px-3 py-2">Protokol</th>
              <th className="px-3 py-2">Driver</th>
              <th className="px-3 py-2">Endpoint</th>
              {editable && <th className="px-3 py-2">Aksi</th>}
            </tr>
          </thead>
          <tbody>
            {(devices.data ?? []).map((d) => (
              <tr key={d.id} className="border-t border-border-secondary">
                <td className="px-3 py-2">{d.name}</td>
                <td className="px-3 py-2">{d.brand}</td>
                <td className="px-3 py-2">{d.model ?? "-"}</td>
                <td className="px-3 py-2">{d.protocol}</td>
                <td className="px-3 py-2">{d.driver}</td>
                <td className="px-3 py-2">{d.endpoint ?? "-"}</td>
                {editable && (
                  <td className="px-3 py-2">
                    <button
                      type="button"
                      className="mr-2 text-text-brand-primary"
                      onClick={() => {
                        setEditId(d.id);
                        setForm({
                          name: d.name,
                          brand: d.brand,
                          model: d.model,
                          serial: d.serial,
                          protocol: d.protocol,
                          driver: d.driver,
                          endpoint: d.endpoint,
                          location_id: d.location_id,
                          active: d.active,
                        });
                        setFormOpen(true);
                      }}
                    >
                      Ubah
                    </button>
                    <button
                      type="button"
                      className="text-text-error"
                      onClick={() => {
                        if (window.confirm(`Hapus perangkat ${d.name}?`)) remove.mutate(d.id);
                      }}
                    >
                      Hapus
                    </button>
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {formOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
          <div className="w-full max-w-md rounded-xl bg-bg-primary p-6">
            <h2 className="mb-4 text-lg font-semibold">{editId ? "Ubah Perangkat" : "Tambah Perangkat"}</h2>
            <div className="space-y-3">
              <label className="block text-sm">
                Nama
                <input
                  className="mt-1 w-full rounded-lg border border-border-secondary bg-bg-primary px-3 py-2"
                  value={form.name}
                  onChange={(e) => setForm({ ...form, name: e.target.value })}
                />
              </label>
              <label className="block text-sm">
                Merek
                <select
                  className="mt-1 w-full rounded-lg border border-border-secondary bg-bg-primary px-3 py-2"
                  value={form.brand}
                  onChange={(e) => setForm({ ...form, brand: e.target.value })}
                >
                  {BRANDS.map((b) => (
                    <option key={b} value={b}>
                      {b}
                    </option>
                  ))}
                </select>
              </label>
              <label className="block text-sm">
                Model
                <input
                  className="mt-1 w-full rounded-lg border border-border-secondary bg-bg-primary px-3 py-2"
                  value={form.model ?? ""}
                  onChange={(e) => setForm({ ...form, model: e.target.value || null })}
                  placeholder="mis. X100-C"
                />
              </label>
              <label className="block text-sm">
                Protokol
                <select
                  className="mt-1 w-full rounded-lg border border-border-secondary bg-bg-primary px-3 py-2"
                  value={form.protocol}
                  onChange={(e) => setForm({ ...form, protocol: e.target.value })}
                >
                  {PROTOCOLS.map((p) => (
                    <option key={p.value} value={p.value}>
                      {p.label}
                    </option>
                  ))}
                </select>
              </label>
              <label className="block text-sm">
                Driver
                <select
                  className="mt-1 w-full rounded-lg border border-border-secondary bg-bg-primary px-3 py-2"
                  value={form.driver ?? ""}
                  onChange={(e) => setForm({ ...form, driver: e.target.value || null })}
                >
                  <option value="">Otomatis per protokol</option>
                  {DRIVERS.map((d) => (
                    <option key={d} value={d}>
                      {d}
                    </option>
                  ))}
                </select>
              </label>
              <label className="block text-sm">
                Endpoint / Host
                <input
                  className="mt-1 w-full rounded-lg border border-border-secondary bg-bg-primary px-3 py-2"
                  value={form.endpoint ?? ""}
                  onChange={(e) => setForm({ ...form, endpoint: e.target.value || null })}
                  placeholder="mis. 192.168.1.50"
                />
              </label>
            </div>
            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                className="rounded-lg border border-border-secondary px-4 py-2 text-sm"
                onClick={() => setFormOpen(false)}
              >
                Batal
              </button>
              <button
                type="button"
                className="rounded-lg bg-bg-brand-primary px-4 py-2 text-sm font-medium text-text-brand-primary"
                onClick={() => save.mutate()}
              >
                Simpan
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
