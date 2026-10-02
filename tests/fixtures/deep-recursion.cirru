
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'f $ %{} 'CodeEntry (:doc |)
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
          :code $ quote $ defn f (n)
            if (= n 0) 0 $ + 1 $ f $ - n 1
        'main! $ %{} 'CodeEntry (:doc |)
          :schema $ :: 'Fn $ {} (:return 'Unit)
          :code $ quote $ defn main! ()
            println "|shallow" $ f 3000
            println "|deep" $ f 1000000
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (println |reload)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
