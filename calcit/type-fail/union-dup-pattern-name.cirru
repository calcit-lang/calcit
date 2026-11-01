
{}
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.one $ %{} 'FileEntry
      :defs $ {}
        'Value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Value (:x 'Number)
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.one (:require)
    'app.two $ %{} 'FileEntry
      :defs $ {}
        'Value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Value (:y 'String)
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.two (:require)
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'Both $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Both 'app.one/Value 'app.two/Value
          :examples $ []
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println Both
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
