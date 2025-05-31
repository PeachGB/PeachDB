public class Utilities {

    public static boolean isWord(String s){
        if (s == null || s.isEmpty()){
            return false;

        }
        return s.matches("[a-zA-z]+");
    }


    public static boolean isNumber(String s){
        if (s == null || s.isEmpty()){
            return false;

        }
        return s.matches("\\d+");
    }
    public static boolean isSymbol(String s){
        if (s == null || s.isEmpty()) {

        return false;
        }
        return s.matches(("[^\\w\\s]+"));
    }
    public static boolean isBoolean(String s){

        return s.equalsIgnoreCase("true") || s.equalsIgnoreCase("false");
    }
}
