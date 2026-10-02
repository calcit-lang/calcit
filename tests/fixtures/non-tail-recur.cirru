
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
        'ok-loop $ %{} 'CodeEntry (:doc |)
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
          :code $ quote $ defn ok-loop ()
            loop ((a 1)) $ if (> a 3) a $ recur $ inc a
        'bad-in-list $ %{} 'CodeEntry (:doc |)
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
          :code $ quote $ defn bad-in-list ()
            loop ((a 1)) $ if (> a 3) a $ [] $ recur $ inc a
        'bad-in-str $ %{} 'CodeEntry (:doc |)
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
          :code $ quote $ defn bad-in-str ()
            loop ((a 1)) $ if (> a 3) a $ str $ recur $ inc a
        'bad-before-tail $ %{} 'CodeEntry (:doc |)
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
          :code $ quote $ defn bad-before-tail ()
            loop ((a 1)) $ if (> a 3) a $ do (recur $ inc a) 100
        'main! $ %{} 'CodeEntry (:doc |)
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :code $ quote $ defn main! ()
            println $ ok-loop
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
