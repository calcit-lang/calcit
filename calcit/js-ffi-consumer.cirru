
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-nil
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-nil.main/main!) (:mode :native) (:reload-fn 'test-nil.main/reload!) (:target :node)
      :feature-policy $ {}
      :modules $ [] |./js-ffi-module/
      :type-slots $ {}
  :files $ {} $ 'test-nil.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            assert= 3 $ plus-one 2
            assert= 4 $ plus-two 2
            assert= 1 $ count-a
            assert= 2 $ count-b
            println |consumer-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (println |reload)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-nil.main
          :require $ app.main :refer $ plus-one plus-two count-a count-b
