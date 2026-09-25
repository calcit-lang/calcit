
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :node)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'base-name $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn base-name (x) x
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} (:file |js-ffi-assets/base-name.js)
              :modules $ {} $ :path |node:path
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
            :features $ #{} :js-ffi
        'count-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-a () 0
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/count.js
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
            :features $ #{} :js-ffi
        'count-b $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-b () 0
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/count.js
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
            :features $ #{} :js-ffi
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            assert= 3 $ plus-one 2
            assert= 4 $ plus-two 2
            println |js-ffi-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'plus-one $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plus-one (x) x
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :inline "|(x)=>x+1"
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
            :features $ #{} :js-ffi
        'plus-two $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plus-two (x) x
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/add-two.js
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
            :features $ #{} :js-ffi
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (println |reload)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
