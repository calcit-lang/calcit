
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-nil
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'test-nil.main/main!
      :mode :native
      :reload-fn 'test-nil.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'test-nil.main
    %{} 'FileEntry
      :defs $ {} $ 'main!
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (log-title "|Testing nil")
            assert= ([]) (.to-list nil)
            assert= ({}) (.to-map nil)
            assert= nil $ .map nil inc
            assert= nil $ .filter nil inc
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-nil.main
          :require $ util.core :refer $ log-title
