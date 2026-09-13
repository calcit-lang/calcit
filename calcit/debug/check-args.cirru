
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |check-args
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'check-args.main/main!) (:mode :native) (:reload-fn 'check-args.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'check-args.main
    %{} 'FileEntry
      :defs $ {}
        'f1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f1 (a) (:: 'Unit)
          :examples $ []
          :schema $ :: 'Dynamic
        'f2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f2 (a ? b)
            hint-fn $ {}
              :args $ [] 'Number $ :: 'Optional 'Number
              :return 'Tuple
            :: :unit
          :examples $ []
          :schema $ :: 'Dynamic
        'f3 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f3 (a & b) (:: 'Unit)
          :examples $ []
          :schema $ :: 'Dynamic
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (; "bad case examples for args checking") (f1 1 4) (f2 1) (f2 1 2) (f2 1 2 4) (f2) (f3 1) (f3 1 2) (f3 1 2 3) (f3)
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns check-args.main
          :require $ [] util.core :refer $ [] log-title inside-eval:
