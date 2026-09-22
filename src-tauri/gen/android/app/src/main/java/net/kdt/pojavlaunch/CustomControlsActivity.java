package net.kdt.pojavlaunch;

import com.zhayi.qookix.R;

import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.view.View;
import android.widget.ArrayAdapter;
import android.widget.ListView;

import androidx.drawerlayout.widget.DrawerLayout;

import net.kdt.pojavlaunch.customcontrols.ControlData;
import net.kdt.pojavlaunch.customcontrols.ControlDrawerData;
import net.kdt.pojavlaunch.customcontrols.ControlJoystickData;
import net.kdt.pojavlaunch.customcontrols.ControlLayout;
import net.kdt.pojavlaunch.customcontrols.EditorExitable;
import net.kdt.pojavlaunch.prefs.LauncherPreferences;

import java.io.IOException;


public class CustomControlsActivity extends androidx.appcompat.app.AppCompatActivity implements EditorExitable {
	private DrawerLayout mDrawerLayout;
	private ListView mDrawerNavigationView;
	private static final int[] MENU_ICONS = {
			R.drawable.ic_qk_plus,     // 添加按键
			R.drawable.ic_qk_plus,     // 添加组合键
			R.drawable.ic_qk_plus,     // 添加摇杆
			R.drawable.ic_qk_folder,   // 加载
			R.drawable.ic_qk_save,     // 保存
			R.drawable.ic_qk_star,     // 选择默认控制布局
			R.drawable.ic_qk_share,    // 导出
	};
	private ControlLayout mControlLayout;

	@Override
	protected void onCreate(Bundle savedInstanceState) {
		super.onCreate(savedInstanceState);

		setContentView(R.layout.activity_custom_controls);

		mControlLayout = findViewById(R.id.customctrl_controllayout);
		mDrawerLayout = findViewById(R.id.customctrl_drawerlayout);
		mDrawerNavigationView = findViewById(R.id.customctrl_navigation_view);
		View mPullDrawerButton = findViewById(R.id.drawer_button);

		mPullDrawerButton.setOnClickListener(v -> mDrawerLayout.openDrawer(mDrawerNavigationView));
		mDrawerLayout.setDrawerLockMode(DrawerLayout.LOCK_MODE_LOCKED_CLOSED);
mDrawerNavigationView.setAdapter(new com.zhayi.qookix.control.QookixMenuAdapter(this,
				com.zhayi.qookix.control.QookixMenuAdapter.entriesOf(this,
						R.array.menu_customcontrol_customactivity, MENU_ICONS)));
		mDrawerNavigationView.setOnItemClickListener((parent, view, position, id) -> {
			switch(position) {
				case 0: mControlLayout.addControlButton(new ControlData("New")); break;
				case 1: mControlLayout.addDrawer(new ControlDrawerData()); break;
				case 2: mControlLayout.addJoystickButton(new ControlJoystickData()); break;
				case 3: mControlLayout.openLoadDialog(); break;
				case 4: mControlLayout.openSaveDialog(this); break;
				case 5: mControlLayout.openSetDefaultDialog(); break;
				case 6: // Saving the currently shown control
					try {
						// QookiX 没有 Pojav 的 scoped.FolderProvider（那个 DocumentsProvider 依赖
			//  @string/storageProviderAuthorities，由 Gradle resValue 生成）。这里改用本项目
			//  已有的 FileProvider（authorities = <applicationId>.fileprovider，
			//  路径映射见 res/xml/file_paths.xml 的 controlmap）。
					String jsonPath = mControlLayout.saveToDirectory(mControlLayout.mLayoutFileName);
					Uri contentUri = androidx.core.content.FileProvider.getUriForFile(
							this, getPackageName() + ".fileprovider", new java.io.File(jsonPath));

						Intent shareIntent = new Intent();
						shareIntent.setAction(Intent.ACTION_SEND);
						shareIntent.putExtra(Intent.EXTRA_STREAM, contentUri);
						shareIntent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
						shareIntent.setType("application/json");

						Intent sendIntent = Intent.createChooser(shareIntent, mControlLayout.mLayoutFileName);
						startActivity(sendIntent);
					}catch (Exception e) {
						Tools.showError(this, e);
					}
					break;
			}
			mDrawerLayout.closeDrawers();
		});
		mControlLayout.setModifiable(true);
		try {
			mControlLayout.loadLayout(LauncherPreferences.PREF_DEFAULTCTRL_PATH);
		}catch (IOException e) {
			Tools.showError(this, e);
		}
	}

	@Override
	public void onBackPressed() {
		mControlLayout.askToExit(this);
	}

	@Override
	public void exitEditor() {
		super.onBackPressed();
	}
}
