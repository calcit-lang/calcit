
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-fn
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'test-fn.main/main!
      :mode :native
      :reload-fn 'test-fn.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'test-fn.main
    %{} 'FileEntry
      :defs $ {} $ 'main!
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (log-title "|Testing fn")
            let
                f1 identity
                f2 &+
                _ $ assert-type f1 $ :: 'Fn
                  {} (:return 'T)
                    :generics $ [] 'T
                    :args $ [] 'T
                _ $ assert-type f2 $ :: 'Fn
                  {} (:return 'Number)
                    :args $ [] 'Number 'Number
              assert= 1 $ f1 1
              assert= 3 $ f2 1 2
              assert= 3 $ apply f2 $ [] 1 2
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-fn.main
          :require $ util.core :refer $ log-title
